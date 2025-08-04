// rag.rs
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::fs;
use chrono::{DateTime, Local};
use crate::models::{ConversationEntry, AppError};

#[derive(Clone)]
pub struct RagSystem {
    db_path: PathBuf,
    pub save_directory: PathBuf,
}

impl RagSystem {
    pub fn new() -> Result<Self, AppError> {
        let save_dir = PathBuf::from("./tourist_data");
        fs::create_dir_all(&save_dir)?;
        
        let db_path = save_dir.join("conversations.db");
        
        // Initialize database
        Self::init_database(&db_path)?;
        
        Ok(Self {
            db_path,
            save_directory: save_dir,
        })
    }

    fn init_database(db_path: &PathBuf) -> Result<(), AppError> {
        let connection = Connection::open(db_path)?;
        
        connection.execute(
            "CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                prompt TEXT NOT NULL,
                response TEXT NOT NULL,
                model_used TEXT NOT NULL,
                response_time_ms INTEGER NOT NULL,
                file_context TEXT
            )",
            [],
        )?;
        
        connection.execute(
            "CREATE TABLE IF NOT EXISTS embeddings_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prompt_hash TEXT UNIQUE NOT NULL,
                prompt TEXT NOT NULL,
                response TEXT NOT NULL,
                similarity_score REAL DEFAULT 0.0
            )",
            [],
        )?;

        Ok(())
    }
    
    pub async fn save_conversation(&self, entry: &ConversationEntry) -> Result<(), AppError> {
        let db_path = self.db_path.clone();
        let entry = entry.clone();
        let save_dir = self.save_directory.clone();
        
        tokio::task::spawn_blocking(move || -> Result<(), AppError> {
            let connection = Connection::open(&db_path)?;
            
            connection.execute(
                "INSERT INTO conversations (timestamp, prompt, response, model_used, response_time_ms, file_context)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry.timestamp.to_rfc3339(),
                    entry.prompt,
                    entry.response,
                    entry.model_used,
                    entry.response_time_ms,
                    entry.file_context.as_deref().unwrap_or("")
                ],
            )?;
            
            // Save as individual text file
            Self::save_as_text_file(&save_dir, &entry)?;
            
            Ok(())
        }).await.map_err(|e| AppError(e.to_string()))??;
        
        Ok(())
    }

    fn save_as_text_file(save_dir: &PathBuf, entry: &ConversationEntry) -> Result<(), AppError> {
        let filename = format!("response_{}.txt", entry.timestamp.format("%Y%m%d_%H%M%S"));
        let file_path = save_dir.join(filename);
        let content = format!(
            "Timestamp: {}\nModel: {}\nResponse Time: {}ms\n\nPrompt:\n{}\n\nResponse:\n{}\n",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
            entry.model_used,
            entry.response_time_ms,
            entry.prompt,
            entry.response
        );
        std::fs::write(file_path, content)?;
        Ok(())
    }
    
    pub async fn find_similar_responses(&self, prompt: &str, limit: usize) -> Result<Vec<ConversationEntry>, AppError> {
        let db_path = self.db_path.clone();
        let prompt = prompt.to_string();
        
        let results = tokio::task::spawn_blocking(move || -> Result<Vec<ConversationEntry>, AppError> {
            let connection = Connection::open(&db_path)?;
            let mut results = Vec::new();
            
            // Simple similarity search using LIKE
            let keywords: Vec<&str> = prompt.split_whitespace().take(3).collect();
            let like_conditions: Vec<String> = keywords.iter()
                .map(|word| format!("(prompt LIKE '%{}%' OR response LIKE '%{}%')", word, word))
                .collect();
            
            let query = format!(
                "SELECT id, timestamp, prompt, response, model_used, response_time_ms, file_context 
                 FROM conversations 
                 WHERE {} 
                 ORDER BY timestamp DESC 
                 LIMIT {}",
                like_conditions.join(" OR "),
                limit
            );
            
            let mut stmt = connection.prepare(&query)?;
            let conversation_iter = stmt.query_map([], |row| {
                let timestamp_str: String = row.get(1)?;
                let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                    .map_err(|_| rusqlite::Error::InvalidColumnType(1, "timestamp".to_string(), rusqlite::types::Type::Text))?
                    .with_timezone(&Local);
                
                let file_context: String = row.get(6)?;
                
                Ok(ConversationEntry {
                    id: row.get(0)?,
                    timestamp,
                    prompt: row.get(2)?,
                    response: row.get(3)?,
                    model_used: row.get(4)?,
                    response_time_ms: row.get(5)?,
                    file_context: if file_context.is_empty() { None } else { Some(file_context) },
                })
            })?;
            
            for conversation in conversation_iter {
                results.push(conversation?);
            }
            
            Ok(results)
        }).await.map_err(|e| AppError(e.to_string()))??;
        
        Ok(results)
    }

    pub fn create_rag_context(&self, suggestions: &[ConversationEntry], current_prompt: &str) -> String {
        if suggestions.is_empty() {
            return current_prompt.to_string();
        }

        let context = suggestions
            .iter()
            .take(2)
            .map(|entry| format!("Previous context:\nQ: {}\nA: {}\n", entry.prompt, entry.response))
            .collect::<Vec<_>>()
            .join("\n");
        
        format!("{}\n\nCurrent question: {}", context, current_prompt)
    }
}