use eframe::egui;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Local;

use crate::models::{ConversationEntry, Analytics, PendingOperation};
use crate::ollama::OllamaClient;
use crate::rag::RagSystem;
use crate::analytics::AnalyticsEngine;
use crate::file_handler::FileHandler;

#[derive(Clone)]
pub struct ChatMessage {
    pub content: String,
    pub is_user: bool,
    pub timestamp: chrono::DateTime<Local>,
    pub model_used: Option<String>,
    pub response_time: Option<i64>,
}

pub struct TouristApp {
    // Core components
    ollama_client: OllamaClient,
    rag_system: Option<RagSystem>,
    analytics_engine: Option<AnalyticsEngine>,
    
    // Chat State
    input_text: String,
    chat_messages: Vec<ChatMessage>,
    is_loading: bool,
    
    // Enhanced Features
    file_content: String,
    file_name: Option<String>,
    
    // Configuration
    model_name: String,
    ollama_url: String,
    enable_rag: bool,
    
    // UI State
    show_sidebar: bool,
    show_settings: bool,
    
    // Data
    analytics: Analytics,
    rag_suggestions: Vec<ConversationEntry>,
    
    // Async handling
    rt: Arc<tokio::runtime::Runtime>,
    pending_operations: Arc<Mutex<Vec<PendingOperation>>>,
    last_response_time: Option<std::time::Instant>,
    
    // Display
    save_directory_display: String,
}

impl Default for TouristApp {
    fn default() -> Self {
        let rag_system = RagSystem::new().ok();
        let analytics_engine = rag_system.as_ref()
            .map(|rag| AnalyticsEngine::new(rag.save_directory.join("conversations.db")));
        
        let save_dir = if let Some(ref rag) = rag_system {
            rag.save_directory.display().to_string()
        } else {
            "Failed to initialize".to_string()
        };
        
        Self {
            ollama_client: OllamaClient::default(),
            rag_system,
            analytics_engine,
            
            input_text: String::new(),
            chat_messages: Vec::new(),
            is_loading: false,
            
            file_content: String::new(),
            file_name: None,
            
            model_name: "deepseek-r1:7b".to_string(),
            ollama_url: "http://localhost:11434/api/generate".to_string(),
            enable_rag: true,
            
            show_sidebar: false,
            show_settings: false,
            
            analytics: Analytics::default(),
            rag_suggestions: Vec::new(),
            
            rt: Arc::new(tokio::runtime::Runtime::new().unwrap()),
            pending_operations: Arc::new(Mutex::new(Vec::new())),
            last_response_time: None,
            
            save_directory_display: save_dir,
        }
    }
}

impl TouristApp {
    fn send_message(&mut self, ctx: &egui::Context) {
        if self.is_loading || self.input_text.trim().is_empty() {
            return;
        }

        // Add user message to chat
        let user_message = ChatMessage {
            content: self.input_text.clone(),
            is_user: true,
            timestamp: Local::now(),
            model_used: None,
            response_time: None,
        };
        self.chat_messages.push(user_message);

        let final_prompt = self.build_final_prompt();
        self.start_generation();
        
        let ollama_client = self.ollama_client.clone();
        let model_name = self.model_name.clone();
        let ctx_clone = ctx.clone();
        let rag_system = self.rag_system.clone();
        let original_prompt = self.input_text.clone();
        let file_context = self.file_name.clone();
        let start_time = std::time::Instant::now();
        let pending_ops = self.pending_operations.clone();
        let rt = self.rt.clone();

        // Clear input immediately
        self.input_text.clear();

        rt.spawn(async move {
            let result = ollama_client.generate_response(&model_name, &final_prompt).await;
            
            match result {
                Ok(response) => {
                    let response_time = start_time.elapsed().as_millis() as i64;
                    
                    // Save to RAG system
                    if let Some(rag) = &rag_system {
                        let entry = ConversationEntry {
                            id: 0,
                            timestamp: Local::now(),
                            prompt: original_prompt,
                            response: response.clone(),
                            model_used: model_name.clone(),
                            response_time_ms: response_time,
                            file_context,
                        };
                        
                        if let Err(e) = rag.save_conversation(&entry).await {
                            eprintln!("Error saving conversation: {}", e);
                        }
                    }
                    
                    let mut ops = pending_ops.lock().await;
                    ops.push(PendingOperation::Response(response));
                    ops.push(PendingOperation::LoadingComplete);
                }
                Err(e) => {
                    let mut ops = pending_ops.lock().await;
                    ops.push(PendingOperation::Error(e.to_string()));
                    ops.push(PendingOperation::LoadingComplete);
                }
            }
            
            ctx_clone.request_repaint();
        });
    }

    fn build_final_prompt(&self) -> String {
        let mut final_prompt = if !self.file_content.is_empty() {
            format!("File context:\n{}\n\nUser message: {}", self.file_content, self.input_text)
        } else {
            self.input_text.clone()
        };

        // Add RAG context if enabled
        if self.enable_rag && !self.rag_suggestions.is_empty() {
            if let Some(rag_system) = &self.rag_system {
                final_prompt = rag_system.create_rag_context(&self.rag_suggestions, &final_prompt);
            }
        }

        final_prompt
    }

    fn start_generation(&mut self) {
        self.is_loading = true;
        self.last_response_time = Some(std::time::Instant::now());
    }

    fn update_rag_suggestions(&mut self) {
        if !self.enable_rag || self.input_text.trim().is_empty() || self.input_text.len() <= 10 {
            return;
        }

        if let Some(rag_system) = &self.rag_system {
            let rag_system = rag_system.clone();
            let prompt = self.input_text.clone();
            let pending_ops = self.pending_operations.clone();
            let rt = self.rt.clone();

            rt.spawn(async move {
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
        if let Some(analytics_engine) = &self.analytics_engine {
            let analytics_engine = analytics_engine.clone();
            let pending_ops = self.pending_operations.clone();
            let rt = self.rt.clone();
            
            rt.spawn(async move {
                match analytics_engine.get_analytics().await {
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
                        let ai_message = ChatMessage {
                            content: result,
                            is_user: false,
                            timestamp: Local::now(),
                            model_used: Some(self.model_name.clone()),
                            response_time: self.last_response_time
                                .map(|t| t.elapsed().as_millis() as i64),
                        };
                        self.chat_messages.push(ai_message);
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
                        let error_message = ChatMessage {
                            content: format!("Error: {}", error),
                            is_user: false,
                            timestamp: Local::now(),
                            model_used: Some("Error".to_string()),
                            response_time: None,
                        };
                        self.chat_messages.push(error_message);
                        self.is_loading = false;
                    }
                }
            }
        }

        // Simple debounced RAG suggestions update
        self.debounced_rag_update();
    }

    fn debounced_rag_update(&mut self) {
        static mut LAST_INPUT: Option<String> = None;
        static mut UPDATE_COUNTER: u32 = 0;
        
        unsafe {
            if LAST_INPUT.as_ref() != Some(&self.input_text) {
                LAST_INPUT = Some(self.input_text.clone());
                UPDATE_COUNTER += 1;
                
                if UPDATE_COUNTER % 5 == 0 {
                    self.update_rag_suggestions();
                }
            }
        }
    }

    fn handle_url_change(&mut self) {
        self.ollama_client.update_url(self.ollama_url.clone());
    }

    fn load_file(&mut self) {
        if let Some(content) = FileHandler::load_text_file() {
            self.file_content = content;
            self.file_name = Some("uploaded_file.txt".to_string()); // You could enhance this to get actual filename
        }
    }

    fn clear_chat(&mut self) {
        self.chat_messages.clear();
    }

    fn export_chat(&self) {
        let chat_content = self.chat_messages
            .iter()
            .map(|msg| {
                let role = if msg.is_user { "User" } else { "Assistant" };
                let timestamp = msg.timestamp.format("%Y-%m-%d %H:%M:%S");
                format!("[{}] {}: {}\n", timestamp, role, msg.content)
            })
            .collect::<String>();

        if let Err(e) = FileHandler::save_text_file(&chat_content, "chat_export.txt") {
            eprintln!("Error saving chat: {}", e);
        }
    }
}

impl eframe::App for TouristApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.set_modern_theme(ctx);
        self.check_async_updates();
        
        if self.is_loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Sidebar
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(280.0)
            .show_animated(ctx, self.show_sidebar, |ui| {
                self.render_sidebar(ui);
            });

        // Main chat area
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_chat_interface(ctx, ui);
        });
    }
}

impl TouristApp {
    fn set_modern_theme(&self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        
        // Modern chat interface colors
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(16, 16, 20);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(32, 33, 35);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 42, 46);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(52, 53, 65);
        visuals.widgets.open.bg_fill = egui::Color32::from_rgb(52, 53, 65);
        
        visuals.override_text_color = Some(egui::Color32::from_rgb(217, 217, 227));
        visuals.window_fill = egui::Color32::from_rgb(16, 16, 20);
        visuals.panel_fill = egui::Color32::from_rgb(16, 16, 20);
        visuals.faint_bg_color = egui::Color32::from_rgb(32, 33, 35);
        visuals.extreme_bg_color = egui::Color32::from_rgb(0, 0, 0);
        
        // Rounded corners for modern look
        visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
        visuals.widgets.active.rounding = egui::Rounding::same(8.0);
        
        ctx.set_visuals(visuals);
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading(egui::RichText::new("🚀 TouristXi9d").size(24.0).color(egui::Color32::from_rgb(99, 102, 241)));
            ui.add_space(20.0);
        });

        ui.separator();
        ui.add_space(16.0);

        // New Chat Button
        if ui.add_sized([260.0, 36.0], egui::Button::new("➕ New Chat")).clicked() {
            self.clear_chat();
        }
        ui.add_space(12.0);

        // Settings Section
        ui.collapsing("⚙️ Settings", |ui| {
            ui.add_space(8.0);
            
            ui.label("Model:");
            ui.text_edit_singleline(&mut self.model_name);
            ui.add_space(8.0);
            
            ui.label("Ollama URL:");
            if ui.text_edit_singleline(&mut self.ollama_url).changed() {
                self.handle_url_change();
            }
            ui.add_space(8.0);
            
            ui.checkbox(&mut self.enable_rag, "🧠 Enable RAG");
        });
        
        ui.add_space(12.0);

        // File Upload Section
        ui.collapsing("📁 File Context", |ui| {
            ui.add_space(8.0);
            
            if ui.button("📎 Attach File").clicked() {
                self.load_file();
            }
            
            if let Some(filename) = self.file_name.clone() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(format!("📄 {}", filename));
                    if ui.small_button("❌").clicked() {
                        self.file_content.clear();
                        self.file_name = None;
                    }
                });
                
                if !self.file_content.is_empty() {
                    ui.add_space(4.0);
                    ui.label(format!("{} characters", self.file_content.len()));
                }
            }
        });

        ui.add_space(12.0);

        // Analytics Section
        ui.collapsing("📊 Analytics", |ui| {
            ui.add_space(8.0);
            
            ui.label(format!("Total Requests: {}", self.analytics.total_requests));
            ui.label(format!("Avg Response: {:.0}ms", self.analytics.avg_response_time));
            ui.label(format!("Model: {}", self.analytics.most_used_model));
            
            ui.add_space(8.0);
            if ui.button("🔄 Refresh").clicked() {
                self.update_analytics();
            }
        });

        ui.add_space(12.0);

        // RAG Suggestions
        if self.enable_rag && !self.rag_suggestions.is_empty() {
            ui.collapsing("🧠 Similar Conversations", |ui| {
                ui.add_space(8.0);
                
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for (i, suggestion) in self.rag_suggestions.iter().take(3).enumerate() {
                            ui.group(|ui| {
                                ui.label(egui::RichText::new(format!("#{}", i + 1)).size(12.0));
                                ui.label(egui::RichText::new(
                                    if suggestion.prompt.len() > 60 {
                                        format!("{}...", &suggestion.prompt[..60])
                                    } else {
                                        suggestion.prompt.clone()
                                    }
                                ).size(11.0));
                            });
                            ui.add_space(4.0);
                        }
                    });
            });
            ui.add_space(12.0);
        }

        // Export Chat
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(16.0);
            if ui.button("💾 Export Chat").clicked() {
                self.export_chat();
            }
        });
    }

    fn render_chat_interface(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Header with hamburger menu
        ui.horizontal(|ui| {
            if ui.button("☰").clicked() {
                self.show_sidebar = !self.show_sidebar;
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(&self.model_name).size(14.0).color(egui::Color32::GRAY));
            });
        });

        ui.separator();

        // Chat messages area
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.chat_messages.is_empty() {
                    self.render_welcome_message(ui);
                } else {
                    self.render_chat_messages(ui);
                }
                
                // Show loading indicator
                if self.is_loading {
                    self.render_loading_message(ui);
                }
            });

        // Input area at bottom
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            self.render_input_area(ctx, ui);
        });
    }

    fn render_welcome_message(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            
            ui.label(egui::RichText::new("🚀").size(48.0));
            ui.add_space(16.0);
            
            ui.label(egui::RichText::new("Welcome to TouristXi9d").size(24.0).strong());
            ui.add_space(8.0);
            
            ui.label(egui::RichText::new("Enhanced AI Client with RAG & Analytics").size(16.0).color(egui::Color32::GRAY));
            ui.add_space(24.0);
            
            ui.label("Start a conversation by typing a message below");
        });
    }

    fn render_chat_messages(&self, ui: &mut egui::Ui) {
        for message in &self.chat_messages {
            ui.add_space(16.0);
            
            if message.is_user {
                self.render_user_message(ui, message);
            } else {
                self.render_assistant_message(ui, message);
            }
        }
        ui.add_space(20.0);
    }

    fn render_user_message(&self, ui: &mut egui::Ui, message: &ChatMessage) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.allocate_ui_with_layout([ui.available_width() * 0.7, 0.0].into(), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(52, 53, 65))
                    .rounding(egui::Rounding::same(12.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&message.content).size(14.0));
                    });
                
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(message.timestamp.format("%H:%M").to_string()).size(11.0).color(egui::Color32::GRAY));
                });
            });
        });
    }

    fn render_assistant_message(&self, ui: &mut egui::Ui, message: &ChatMessage) {
        ui.horizontal(|ui| {
            // Avatar
            ui.add_space(8.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(99, 102, 241))
                .rounding(egui::Rounding::same(16.0))
                .show(ui, |ui| {
                    ui.add_sized([32.0, 32.0], egui::Label::new(egui::RichText::new("🤖").size(16.0)));
                });
            
            ui.add_space(12.0);
            
            // Message content
            ui.allocate_ui_with_layout([ui.available_width() * 0.7, 0.0].into(), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(32, 33, 35))
                    .rounding(egui::Rounding::same(12.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&message.content).size(14.0));
                    });
                
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(message.timestamp.format("%H:%M").to_string()).size(11.0).color(egui::Color32::GRAY));
                    
                    if let Some(model) = &message.model_used {
                        ui.label(egui::RichText::new("•").size(11.0).color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new(model).size(11.0).color(egui::Color32::GRAY));
                    }
                    
                    if let Some(response_time) = message.response_time {
                        ui.label(egui::RichText::new("•").size(11.0).color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new(format!("{}ms", response_time)).size(11.0).color(egui::Color32::GRAY));
                    }
                });
            });
        });
    }

    fn render_loading_message(&self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(99, 102, 241))
                .rounding(egui::Rounding::same(16.0))
                .show(ui, |ui| {
                    ui.add_sized([32.0, 32.0], egui::Label::new(egui::RichText::new("🤖").size(16.0)));
                });
            
            ui.add_space(12.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(32, 33, 35))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label("Thinking...");
                        
                        if let Some(start_time) = self.last_response_time {
                            ui.label(format!("{}ms", start_time.elapsed().as_millis()));
                        }
                    });
                });
        });
    }

    fn render_input_area(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(32, 33, 35))
            .rounding(egui::Rounding::same(16.0))
            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // File attachment indicator
                    if self.file_name.is_some() {
                        ui.label(egui::RichText::new("📎").color(egui::Color32::from_rgb(99, 102, 241)));
                    }
                    
                    // Text input
                    let response = egui::TextEdit::multiline(&mut self.input_text)
                        .desired_width(ui.available_width() - 60.0)
                        .desired_rows(1)
                        .hint_text("Type your message...")
                        .show(ui);
                    
                    // Handle Enter key
                    if response.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
                        self.send_message(ctx);
                    }
                    
                    ui.add_space(8.0);
                    
                    // Send button
                    let send_button = egui::Button::new("🚀")
                        .fill(egui::Color32::from_rgb(99, 102, 241))
                        .rounding(egui::Rounding::same(8.0));
                    
                    if ui.add_enabled(!self.is_loading && !self.input_text.trim().is_empty(), send_button).clicked() {
                        self.send_message(ctx);
                    }
                });
            });
    }
}