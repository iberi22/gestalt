// MiniMax LLM Provider for Gestalt Rust
// Integrates with MiniMax Chat API v2

use async_trait::async_trait;
use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;

use crate::ports::outbound::llm_provider::{
    LlmError, LlmProvider, LlmRequest, LlmResponse, LlmUsage, LlmMessage,
};
use futures::stream::{self, StreamExt};

#[derive(Debug, Clone)]
pub struct MiniMaxProvider {
    client: Client,
    model: String,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MiniMaxRequest {
    model: String,
    messages: Vec<MiniMaxMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    top_p: Option<f32>,
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxChoice {
    message: MiniMaxMessage,
}

#[derive(Debug, Deserialize)]
struct MiniMaxResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<MiniMaxChoice>,
    usage: Option<MiniMaxUsage>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl MiniMaxProvider {
    /// Create a new MiniMax provider
    pub fn new(model: String, api_key: Option<String>) -> Self {
        let api_key = api_key.unwrap_or_else(|| {
            env::var("MINIMAX_API_KEY").unwrap_or_else(|_| {
                panic!("MINIMAX_API_KEY environment variable not set")
            })
        });

        Self {
            client: Client::new(),
            model,
            api_key,
            base_url: "https://api.minimax.chat/v1/text".to_string(),
        }
    }

    /// Create with default model
    pub fn default() -> Self {
        let model = env::var("MINIMAX_MODEL")
            .unwrap_or_else(|_| "MiniMax-M2.1".to_string());
        
        Self::new(model, None)
    }
}

#[async_trait]
impl LlmProvider for MiniMaxProvider {
    fn name(&self) -> String {
        format!("minimax/{}", self.model)
    }

    fn supported_models(&self) -> Vec<String> {
        vec!["MiniMax-M2.1".to_string(), "MiniMax-M2".to_string()]
    }

    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/chatcompletion_v2", self.base_url);

        let messages: Vec<MiniMaxMessage> = request
            .messages
            .into_iter()
            .map(|m| MiniMaxMessage {
                role: m.role.unwrap_or_else(|| "user".to_string()),
                content: m.content,
            })
            .collect();

        let body = MiniMaxRequest {
            model: self.model.clone(),
            messages,
            temperature: Some(request.temperature),
            max_tokens: Some(request.max_tokens),
            top_p: None,
            stream: Some(false),
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("MiniMax API error: {}", error_text)));
        }

        let minimax_resp: MiniMaxResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let content = minimax_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let usage = minimax_resp.usage.map(|u| LlmUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(LlmResponse {
            content,
            usage,
            model: self.model.clone(),
        })
    }

    async fn stream(&self, request: LlmRequest) -> Result<BoxStream<'static, Result<String, LlmError>>, LlmError> {
        let url = format!("{}/chatcompletion_v2", self.base_url);

        let messages: Vec<MiniMaxMessage> = request
            .messages
            .into_iter()
            .map(|m| MiniMaxMessage {
                role: m.role.unwrap_or_else(|| "user".to_string()),
                content: m.content,
            })
            .collect();

        let body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "stream": true
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let mut stream = response.bytes_stream();
        let client = self.client.clone();
        let api_key = self.api_key.clone();

        let stream = stream::unfold((), async move |_| {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    // Parse SSE format from MiniMax
                    if let Some(content) = extract_sse_content(&chunk_str) {
                        Some((Ok(content), ()))
                    } else {
                        Some((Ok(String::new()), ()))
                    }
                }
                Some(Err(e)) => Some((Err(LlmError::Network(e.to_string())), ())),
                None => None,
            }
        });

        Ok(stream.boxed())
    }
}

/// Extract content from SSE response
fn extract_sse_content(sse: &str) -> Option<String> {
    // MiniMax streaming format: data: {...}
    for line in sse.lines() {
        if line.trim_start().starts_with("data:") {
            let json = line.trim_start().trim_start_matches("data:");
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                if let Some(choice) = value.get("choices").and_then(|c| c.as_array()).and_then(|c| c.first()) {
                    if let Some(content) = choice.get("delta").and_then(|d| d.get("content")) {
                        return Some(content.as_str().unwrap_or("").to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_creation() {
        // Skip if no API key
        if env::var("MINIMAX_API_KEY").is_err() {
            return;
        }

        let provider = MiniMaxProvider::default();
        assert_eq!(provider.name(), "minimax/MiniMax-M2.1");
    }

    #[tokio::test]
    async fn test_supported_models() {
        let provider = MiniMaxProvider::new("test-model".to_string(), Some("test-key".to_string());
        let models = provider.supported_models();
        assert!(models.contains(&"MiniMax-M2.1".to_string()));
    }
}
