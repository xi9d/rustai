// plugins/mod.rs - Example plugin system
use crate::models::{ConversationEntry, AppError};
use async_trait::async_trait;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    async fn process(&self, input: &str) -> Result<String, AppError>;
    fn is_enabled(&self) -> bool;
}

// plugins/translator.rs - Example translation plugin
pub struct TranslatorPlugin {
    enabled: bool,
    target_language: String,
}

impl TranslatorPlugin {
    pub fn new(target_language: String) -> Self {
        Self {
            enabled: true,
            target_language,
        }
    }
}

#[async_trait]
impl Plugin for TranslatorPlugin {
    fn name(&self) -> &str {
        "Translator"
    }

    async fn process(&self, input: &str) -> Result<String, AppError> {
        // Mock translation - in real implementation, you'd call a translation API
        Ok(format!("[Translated to {}]: {}", self.target_language, input))
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// plugins/summarizer.rs - Example summarization plugin
pub struct SummarizerPlugin {
    enabled: bool,
    max_length: usize,
}

impl SummarizerPlugin {
    pub fn new(max_length: usize) -> Self {
        Self {
            enabled: true,
            max_length,
        }
    }
}

#[async_trait]
impl Plugin for SummarizerPlugin {
    fn name(&self) -> &str {
        "Summarizer"
    }

    async fn process(&self, input: &str) -> Result<String, AppError> {
        if input.len() <= self.max_length {
            return Ok(input.to_string());
        }
        
        // Simple truncation - in real implementation, use proper summarization
        Ok(format!("{}...", &input[..self.max_length]))
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// Enhanced TouristApp with plugin support
use std::collections::HashMap;

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        let name = plugin.name().to_string();
        self.plugins.insert(name, plugin);
    }

    pub async fn process_with_plugins(&self, input: &str) -> Result<String, AppError> {
        let mut result = input.to_string();
        
        for plugin in self.plugins.values() {
            if plugin.is_enabled() {
                result = plugin.process(&result).await?;
            }
        }
        
        Ok(result)
    }

    pub fn get_plugin_names(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }
}

// Usage example in main app:
impl TouristApp {
    pub fn with_plugins() -> Self {
        let mut app = Self::default();
        
        // Initialize plugin manager
        let mut plugin_manager = PluginManager::new();
        
        // Register plugins
        plugin_manager.register_plugin(Box::new(TranslatorPlugin::new("Spanish".to_string())));
        plugin_manager.register_plugin(Box::new(SummarizerPlugin::new(500)));
        
        // Store plugin manager in app (you'd add this field to TouristApp)
        // app.plugin_manager = Some(plugin_manager);
        
        app
    }
}