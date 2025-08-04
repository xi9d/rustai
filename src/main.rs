use eframe::egui;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Local};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::fs;

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Clone, Debug)]
struct ConversationEntry {
    id: i64,
    timestamp: DateTime<Local>,
    prompt: String,
    response: String,
    model_used: String,
    response_time_ms: i64,
    file_context: Option<String>,
}

#[derive(Default, Clone, Debug)]
struct Analytics {
    total_requests: usize,
    avg_response_time: f64,
    most_used_model: String,
    total_tokens_approx: usize,
    sessions_today: usize,
    cache_hits: usize,
    cache_misses: usize,
}

#[derive(Clone)]
struct RagSystem {
    db_path: PathBuf,
    save_directory: PathBuf,
}

// Custom error type that implements Send
#[derive(Debug)]
struct AppError(String);

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

impl RagSystem {
    fn new() -> Result<Self, AppError> {
        let save_dir = PathBuf::from("./tourist_data");
        fs::create_dir_all(&save_dir)?;
        
        let db_path = save_dir.join("conversations.db");
        
        // Initialize database with connection that gets dropped
        {
            let connection = Connection::open(&db_path)?;
            
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
        }
        
        Ok(Self {
            db_path,
            save_directory: save_dir,
        })
    }
    
    async fn save_conversation(&self, entry: &ConversationEntry) -> Result<(), AppError> {
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
            
            // Also save as individual text file
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
        }).await.map_err(|e| AppError(e.to_string()))??;
        
        Ok(())
    }
    
    async fn find_similar_responses(&self, prompt: &str, limit: usize) -> Result<Vec<ConversationEntry>, AppError> {
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
    
    async fn get_analytics(&self) -> Result<Analytics, AppError> {
        let db_path = self.db_path.clone();
        
        let analytics = tokio::task::spawn_blocking(move || -> Result<Analytics, AppError> {
            let connection = Connection::open(&db_path)?;
            let mut analytics = Analytics::default();
            
            // Total requests
            let mut stmt = connection.prepare("SELECT COUNT(*) FROM conversations")?;
            let total: i64 = stmt.query_row([], |row| row.get(0))?;
            analytics.total_requests = total as usize;
            
            // Average response time
            let mut stmt = connection.prepare("SELECT AVG(response_time_ms) FROM conversations")?;
            analytics.avg_response_time = stmt.query_row([], |row| {
                let avg: Option<f64> = row.get(0)?;
                Ok(avg.unwrap_or(0.0))
            })?;
            
            // Most used model
            let mut stmt = connection.prepare(
                "SELECT model_used, COUNT(*) as count FROM conversations GROUP BY model_used ORDER BY count DESC LIMIT 1"
            )?;
            analytics.most_used_model = stmt.query_row([], |row| {
                let model: String = row.get(0)?;
                Ok(model)
            }).unwrap_or_default();
            
            // Sessions today
            let today = Local::now().format("%Y-%m-%d").to_string();
            let mut stmt = connection.prepare(
                "SELECT COUNT(DISTINCT DATE(timestamp)) FROM conversations WHERE DATE(timestamp) = ?1"
            )?;
            let sessions: i64 = stmt.query_row([&today], |row| row.get(0))?;
            analytics.sessions_today = sessions as usize;
            
            // Approximate token count
            let mut stmt = connection.prepare("SELECT SUM(LENGTH(prompt) + LENGTH(response)) FROM conversations")?;
            let total_chars: Option<i64> = stmt.query_row([], |row| row.get(0))?;
            analytics.total_tokens_approx = total_chars.unwrap_or(0) as usize / 4;
            
            Ok(analytics)
        }).await.map_err(|e| AppError(e.to_string()))??;
        
        Ok(analytics)
    }
}

struct TouristApp {
    input_text: String,
    output_text: String,
    is_loading: bool,
    client: Client,
    rt: Arc<tokio::runtime::Runtime>,
    model_name: String,
    ollama_url: String,
    file_content: String,
    rag_system: Option<RagSystem>,
    analytics: Analytics,
    show_analytics: bool,
    show_rag_suggestions: bool,
    rag_suggestions: Vec<ConversationEntry>,
    last_response_time: Option<std::time::Instant>,
    conversation_history: Vec<ConversationEntry>,
    enable_rag: bool,
    save_directory_display: String,
    pending_operations: Arc<Mutex<Vec<PendingOperation>>>,
}

#[derive(Debug)]
enum PendingOperation {
    Response(String),
    Analytics(Analytics),
    RagSuggestions(Vec<ConversationEntry>),
    LoadingComplete,
    Error(String),
}

impl Default for TouristApp {
    fn default() -> Self {
        let rag_system = RagSystem::new().ok();
        let save_dir = if let Some(ref rag) = rag_system {
            rag.save_directory.display().to_string()
        } else {
            "Failed to initialize".to_string()
        };
        
        Self {
            input_text: String::new(),
            output_text: String::new(),
            is_loading: false,
            client: Client::new(),
            rt: Arc::new(tokio::runtime::Runtime::new().unwrap()),
            model_name: "deepseek-r1:7b".to_string(),
            ollama_url: "http://localhost:11434/api/generate".to_string(),
            file_content: String::new(),
            rag_system,
            analytics: Analytics::default(),
            show_analytics: true,
            show_rag_suggestions: true,
            rag_suggestions: Vec::new(),
            last_response_time: None,
            conversation_history: Vec::new(),
            enable_rag: true,
            save_directory_display: save_dir,
            pending_operations: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl TouristApp {
    fn ask_ollama(&mut self, ctx: &egui::Context) {
        if self.is_loading {
            return;
        }

        let mut final_prompt = if !self.file_content.is_empty() {
            format!("{}\n\n{}", self.file_content, self.input_text)
        } else {
            self.input_text.clone()
        };

        // Add RAG context if enabled
        if self.enable_rag && !self.rag_suggestions.is_empty() {
            let context = self.rag_suggestions
                .iter()
                .take(2)
                .map(|entry| format!("Previous context:\nQ: {}\nA: {}\n", entry.prompt, entry.response))
                .collect::<Vec<_>>()
                .join("\n");
            final_prompt = format!("{}\n\nCurrent question: {}", context, self.input_text);
        }

        if final_prompt.trim().is_empty() {
            return;
        }

        self.is_loading = true;
        self.output_text = "Generating response...".to_string();
        self.last_response_time = Some(std::time::Instant::now());

        let client = self.client.clone();
        let ollama_url = self.ollama_url.clone();
        let model_name = self.model_name.clone();
        let ctx_clone = ctx.clone();
        let rag_system = self.rag_system.clone();
        let original_prompt = self.input_text.clone();
        let file_context = if self.file_content.is_empty() { None } else { Some(self.file_content.clone()) };
        let start_time = std::time::Instant::now();
        let pending_ops = self.pending_operations.clone();

        self.rt.spawn(async move {
            let request = OllamaRequest {
                model: model_name.clone(),
                prompt: final_prompt,
                stream: false,
            };

            let result = match client.post(&ollama_url).json(&request).send().await {
                Ok(response) => {
                    match response.json::<OllamaResponse>().await {
                        Ok(ollama_resp) => {
                            let response_time = start_time.elapsed().as_millis() as i64;
                            
                            // Save to RAG system
                            if let Some(rag) = &rag_system {
                                let entry = ConversationEntry {
                                    id: 0,
                                    timestamp: Local::now(),
                                    prompt: original_prompt,
                                    response: ollama_resp.response.clone(),
                                    model_used: model_name,
                                    response_time_ms: response_time,
                                    file_context,
                                };
                                
                                if let Err(e) = rag.save_conversation(&entry).await {
                                    eprintln!("Error saving conversation: {}", e);
                                }
                            }
                            
                            ollama_resp.response
                        },
                        Err(e) => format!("Error parsing response: {}", e),
                    }
                }
                Err(e) => format!("Error making request: {}", e),
            };

            // Queue the result
            {
                let mut ops = pending_ops.lock().await;
                ops.push(PendingOperation::Response(result));
                ops.push(PendingOperation::LoadingComplete);
            }
            
            ctx_clone.request_repaint();
        });
    }

    fn update_rag_suggestions(&mut self) {
        if !self.enable_rag || self.input_text.trim().is_empty() {
            return;
        }

        if let Some(rag_system) = &self.rag_system {
            let rag_system = rag_system.clone();
            let prompt = self.input_text.clone();
            let pending_ops = self.pending_operations.clone();

            self.rt.spawn(async move {
                match rag_system.find_similar_responses(&prompt, 3).await {
                    Ok(suggestions) => {
                        let mut ops = pending_ops.lock().await;
                        ops.push(PendingOperation::RagSuggestions(suggestions));
                    }
                    Err(e) => {
                        let mut ops = pending_ops.lock().await;
                        ops.push(PendingOperation::Error(format!("RAG error: {}", e)));
                    }
                }
            });
        }
    }

    fn update_analytics(&mut self) {
        if let Some(rag_system) = &self.rag_system {
            let rag_system = rag_system.clone();
            let pending_ops = self.pending_operations.clone();
            
            self.rt.spawn(async move {
                match rag_system.get_analytics().await {
                    Ok(analytics) => {
                        let mut ops = pending_ops.lock().await;
                        ops.push(PendingOperation::Analytics(analytics));
                    }
                    Err(e) => {
                        let mut ops = pending_ops.lock().await;
                        ops.push(PendingOperation::Error(format!("Analytics error: {}", e)));
                    }
                }
            });
        }
    }

    fn check_async_updates(&mut self) {
        if let Ok(mut ops) = self.pending_operations.try_lock() {
            for op in ops.drain(..) {
                match op {
                    PendingOperation::Response(result) => {
                        self.output_text = result;
                    }
                    PendingOperation::Analytics(analytics) => {
                        self.analytics = analytics;
                    }
                    PendingOperation::RagSuggestions(suggestions) => {
                        self.rag_suggestions = suggestions;
                    }
                    PendingOperation::LoadingComplete => {
                        self.is_loading = false;
                    }
                    PendingOperation::Error(error) => {
                        eprintln!("Background error: {}", error);
                    }
                }
            }
        }

        // Update RAG suggestions when input changes (simple debouncing)
        if !self.input_text.trim().is_empty() && self.enable_rag && self.input_text.len() > 10 {
            // Use a simple counter-based debouncing mechanism
            static mut LAST_INPUT: Option<String> = None;
            static mut UPDATE_COUNTER: u32 = 0;
            
            unsafe {
                if LAST_INPUT.as_ref() != Some(&self.input_text) {
                    LAST_INPUT = Some(self.input_text.clone());
                    UPDATE_COUNTER += 1;
                    
                    // Only update every few input changes to avoid spam
                    if UPDATE_COUNTER % 5 == 0 {
                        self.update_rag_suggestions();
                    }
                }
            }
        }
    }
}

impl eframe::App for TouristApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set custom visuals for a modern dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(20, 20, 20);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 30);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 40, 40);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(50, 50, 50);
        visuals.override_text_color = Some(egui::Color32::WHITE);
        visuals.window_fill = egui::Color32::from_rgb(10, 10, 10);
        visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50));
        visuals.extreme_bg_color = egui::Color32::from_rgb(20, 20, 20);
        ctx.set_visuals(visuals);

        self.check_async_updates();
        
        if self.is_loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Main heading with custom styling
            ui.heading(egui::RichText::new("🚀 TouristXi9d - Enhanced AI Client with RAG & Analytics").size(24.0).color(egui::Color32::LIGHT_BLUE));
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Configuration row
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Model:").size(16.0));
                ui.text_edit_singleline(&mut self.model_name);
                
                ui.add_space(10.0);
                
                ui.label(egui::RichText::new("Ollama URL:").size(16.0));
                ui.text_edit_singleline(&mut self.ollama_url);
                
                ui.add_space(10.0);
                
                ui.checkbox(&mut self.enable_rag, "Enable RAG");
                ui.checkbox(&mut self.show_analytics, "Show Analytics");
            });
            
            ui.add_space(10.0);
            
            // Save directory info
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("💾 Save Directory:").size(16.0));
                ui.label(&self.save_directory_display);
            });
            
            ui.add_space(10.0);
            
            // Main content area with side panel for analytics
            egui::SidePanel::right("analytics_panel")
                .resizable(true)
                .show_animated(ctx, self.show_analytics, |ui| {
                    ui.heading(egui::RichText::new("📊 Analytics").size(20.0).color(egui::Color32::LIGHT_BLUE));
                    
                    ui.group(|ui| {
                        ui.label(format!("Total Requests: {}", self.analytics.total_requests));
                        ui.label(format!("Avg Response Time: {:.1}ms", self.analytics.avg_response_time));
                        ui.label(format!("Most Used Model: {}", self.analytics.most_used_model));
                        ui.label(format!("Approx Tokens: {}", self.analytics.total_tokens_approx));
                        ui.label(format!("Sessions Today: {}", self.analytics.sessions_today));
                        ui.label(format!("Cache Hits: {}", self.analytics.cache_hits));
                        ui.label(format!("Cache Misses: {}", self.analytics.cache_misses));
                    });
                    
                    if ui.button("🔄 Refresh Analytics").clicked() {
                        self.update_analytics();
                    }
                    
                    ui.add_space(10.0);
                    
                    if self.enable_rag && !self.rag_suggestions.is_empty() {
                        ui.heading(egui::RichText::new("🧠 RAG Suggestions").size(18.0).color(egui::Color32::LIGHT_BLUE));
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for (i, suggestion) in self.rag_suggestions.iter().enumerate() {
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(format!("Similar #{}", i + 1)).size(16.0));
                                        ui.label(format!("Time: {}", suggestion.timestamp.format("%m/%d %H:%M")));
                                        ui.label(format!("Prompt: {}", 
                                            if suggestion.prompt.len() > 50 {
                                                format!("{}...", &suggestion.prompt[..50])
                                            } else {
                                                suggestion.prompt.clone()
                                            }
                                        ));
                                    });
                                }
                            });
                    }
                });
            
            // File loading section
            ui.horizontal(|ui| {
                if ui.button("📁 Load Text File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Text files", &["txt", "md", "rs", "py", "js", "json"])
                        .pick_file()
                    {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                self.file_content = content;
                            }
                            Err(e) => {
                                self.output_text = format!("Error reading file: {}", e);
                            }
                        }
                    }
                }
                
                if !self.file_content.is_empty() {
                    ui.label(format!("📄 File loaded ({} chars)", self.file_content.len()));
                    if ui.button("❌ Clear File").clicked() {
                        self.file_content.clear();
                    }
                }
            });
            
            if !self.file_content.is_empty() {
                ui.collapsing("File Content Preview", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .show(ui, |ui| {
                            ui.text_edit_multiline(&mut self.file_content);
                        });
                });
            }
            
            ui.add_space(15.0);
            
            // Input section
            ui.label(egui::RichText::new("Input Prompt:").size(18.0).strong());
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.text_edit_multiline(&mut self.input_text);
                });
            
            ui.horizontal(|ui| {
                if ui.button("🚀 Generate Response").clicked() && !self.is_loading {
                    self.ask_ollama(ctx);
                }
                
                if ui.button("🗑 Clear Input").clicked() {
                    self.input_text.clear();
                }
                
                if self.is_loading {
                    ui.spinner();
                    ui.label("Generating...");
                    if let Some(start_time) = self.last_response_time {
                        ui.label(format!("{}ms", start_time.elapsed().as_millis()));
                    }
                }
            });
            
            ui.add_space(15.0);
            
            // Output section
            ui.label(egui::RichText::new("Output:").size(18.0).strong());
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut self.output_text)
                        .desired_width(f32::INFINITY)
                        .show(ui);
                });
            
            ui.horizontal(|ui| {
                if ui.button("📋 Copy Output").clicked() {
                    ui.output_mut(|o| o.copied_text = self.output_text.clone());
                }
                
                if ui.button("🗑 Clear Output").clicked() {
                    self.output_text.clear();
                }
                
                if ui.button("💾 Save Output").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("ollama_output.txt")
                        .save_file()
                    {
                        if let Err(e) = std::fs::write(&path, &self.output_text) {
                            eprintln!("Error saving file: {}", e);
                        }
                    }
                }
                
                if ui.button("📂 Open Save Directory").clicked() {
                    if let Some(rag) = &self.rag_system {
                        #[cfg(target_os = "windows")]
                        std::process::Command::new("explorer")
                            .arg(&rag.save_directory)
                            .spawn()
                            .ok();
                        
                        #[cfg(target_os = "macos")]
                        std::process::Command::new("open")
                            .arg(&rag.save_directory)
                            .spawn()
                            .ok();
                        
                        #[cfg(target_os = "linux")]
                        std::process::Command::new("xdg-open")
                            .arg(&rag.save_directory)
                            .spawn()
                            .ok();
                    }
                }
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "TouristXi9d - Enhanced AI Client with RAG & Analytics",
        options,
        Box::new(|_cc| Ok(Box::new(TouristApp::default()))),
    )
}