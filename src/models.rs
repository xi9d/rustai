// models.rs
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OllamaRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
}

#[derive(Deserialize)]
pub struct OllamaResponse {
    pub response: String,
}

#[derive(Clone, Debug)]
pub struct ConversationEntry {
    pub id: i64,
    pub timestamp: DateTime<Local>,
    pub prompt: String,
    pub response: String,
    pub model_used: String,
    pub response_time_ms: i64,
    pub file_context: Option<String>,
}

#[derive(Default, Clone, Debug)]
pub struct Analytics {
    pub total_requests: usize,
    pub avg_response_time: f64,
    pub most_used_model: String,
    pub total_tokens_approx: usize,
    pub sessions_today: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

#[derive(Debug)]
pub enum PendingOperation {
    Response(String),
    Analytics(Analytics),
    RagSuggestions(Vec<ConversationEntry>),
    LoadingComplete,
    Error(String),
}

// Custom error type that implements Send
#[derive(Debug)]
pub struct AppError(pub String);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError(err.to_string())
    }
}

impl From<chrono::ParseError> for AppError {
    fn from(err: chrono::ParseError) -> Self {
        AppError(err.to_string())
    }
}