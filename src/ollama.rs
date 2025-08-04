// ollama.rs
use reqwest::Client;
use crate::models::{OllamaRequest, OllamaResponse, AppError};

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
}

impl OllamaClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    pub async fn generate_response(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<String, AppError> {
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError(format!("Request failed: {}", e)))?;

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| AppError(format!("Failed to parse response: {}", e)))?;

        Ok(ollama_response.response)
    }

    pub fn update_url(&mut self, new_url: String) {
        self.base_url = new_url;
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new("http://localhost:11434/api/generate".to_string())
    }
}