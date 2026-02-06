//! MiniMax LLM Provider
//!
//! Integration with MiniMax API for chat completions.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::env;

use crate::ports::outbound::llm_provider::{LlmError, LlmProvider, LlmRequest, LlmResponse};

/// MiniMax LLM Provider
#[derive(Clone, Debug)]
pub struct MiniMaxProvider {
    /// HTTP client
    client: Client,
    /// Model identifier
    model: String,
    /// API key
    api_key: String,
    /// Base URL for API
    base_url: String,
}

impl MiniMaxProvider {
    /// Create a new MiniMax provider
    pub fn new(model: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            model,
            api_key,
            base_url: "https://api.minimax.chat/v1/text".to_string(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, LlmError> {
        let model = env::var("MINIMAX_MODEL")
            .unwrap_or_else(|_| "MiniMax-M2".to_string());
        
        let api_key = env::var("MINIMAX_API_KEY")
            .map_err(|_| LlmError::Auth("MINIMAX_API_KEY not set".to_string()))?;
        
        Ok(Self::new(model, api_key))
    }
}

#[async_trait]
impl LlmProvider for MiniMaxProvider {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/chatcompletion_v2", self.base_url);

        let body = json!({
            "model": self.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096)
        });

        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!(
                "MiniMax Error {}: {}", status, text
            )));
        }

        let resp_json: serde_json::Value = resp.json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| LlmError::Parse("Invalid MiniMax response structure".to_string()))?
            .to_string();

        Ok(LlmResponse {
            content,
            usage: None,
        })
    }

    async fn stream(
        &self,
        _request: LlmRequest
    ) -> Result<futures::stream::BoxStream<'static, Result<String, LlmError>>, LlmError> {
        Err(LlmError::Provider("Streaming not implemented yet for MiniMax".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_creation() {
        // Test creating provider from env (will fail without env vars)
        let result = MiniMaxProvider::from_env();
        assert!(result.is_err()); // Expects MINIMAX_API_KEY
    }
}
