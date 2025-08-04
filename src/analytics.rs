// analytics.rs
use rusqlite::Connection;
use chrono::Local;
use crate::models::{Analytics, AppError};
use std::path::PathBuf;

#[derive(Clone)]
pub struct AnalyticsEngine {
    db_path: PathBuf,
}

impl AnalyticsEngine {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub async fn get_analytics(&self) -> Result<Analytics, AppError> {
        let db_path = self.db_path.clone();
        
        let analytics = tokio::task::spawn_blocking(move || -> Result<Analytics, AppError> {
            let connection = Connection::open(&db_path)?;
            let mut analytics = Analytics::default();
            
            // Total requests
            analytics.total_requests = Self::get_total_requests(&connection)?;
            
            // Average response time
            analytics.avg_response_time = Self::get_avg_response_time(&connection)?;
            
            // Most used model
            analytics.most_used_model = Self::get_most_used_model(&connection)?;
            
            // Sessions today
            analytics.sessions_today = Self::get_sessions_today(&connection)?;
            
            // Approximate token count
            analytics.total_tokens_approx = Self::get_token_count(&connection)?;
            
            Ok(analytics)
        }).await.map_err(|e| AppError(e.to_string()))??;
        
        Ok(analytics)
    }

    fn get_total_requests(connection: &Connection) -> Result<usize, AppError> {
        let mut stmt = connection.prepare("SELECT COUNT(*) FROM conversations")?;
        let total: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(total as usize)
    }

    fn get_avg_response_time(connection: &Connection) -> Result<f64, AppError> {
        let mut stmt = connection.prepare("SELECT AVG(response_time_ms) FROM conversations")?;
        let avg = stmt.query_row([], |row| {
            let avg: Option<f64> = row.get(0)?;
            Ok(avg.unwrap_or(0.0))
        })?;
        Ok(avg)
    }

    fn get_most_used_model(connection: &Connection) -> Result<String, AppError> {
        let mut stmt = connection.prepare(
            "SELECT model_used, COUNT(*) as count FROM conversations GROUP BY model_used ORDER BY count DESC LIMIT 1"
        )?;
        let model = stmt.query_row([], |row| {
            let model: String = row.get(0)?;
            Ok(model)
        }).unwrap_or_default();
        Ok(model)
    }

    fn get_sessions_today(connection: &Connection) -> Result<usize, AppError> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let mut stmt = connection.prepare(
            "SELECT COUNT(DISTINCT DATE(timestamp)) FROM conversations WHERE DATE(timestamp) = ?1"
        )?;
        let sessions: i64 = stmt.query_row([&today], |row| row.get(0))?;
        Ok(sessions as usize)
    }

    fn get_token_count(connection: &Connection) -> Result<usize, AppError> {
        let mut stmt = connection.prepare("SELECT SUM(LENGTH(prompt) + LENGTH(response)) FROM conversations")?;
        let total_chars: Option<i64> = stmt.query_row([], |row| row.get(0))?;
        Ok(total_chars.unwrap_or(0) as usize / 4) // Rough approximation
    }
}