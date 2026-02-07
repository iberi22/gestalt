use async_trait::async_trait;
use futures::stream::BoxStream;
use thiserror::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Parsing error: {0}")]
    Parse(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Context length exceeded")]
    ContextLength,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct LlmMessage {
    pub role: Option<String>,
    pub content: String,
}

impl LlmMessage {
    pub fn user(content: &str) -> Self {
        Self {
            role: Some("user".to_string()),
            content: content.to_string(),
        }
    }

    pub fn system(content: &str) -> Self {
        Self {
            role: Some("system".to_string()),
            content: content.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for LlmRequest {
    fn default() -> Self {
        Self {
            messages: vec![],
            model: "MiniMax-M2.1".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub usage: Option<LlmUsage>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a single response from the LLM
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;

    /// Stream the response from the LLM
    async fn stream(&self, request: LlmRequest) -> Result<BoxStream<'static, Result<String, LlmError>>, LlmError>;

    /// Get provider name
    fn name(&self) -> String;

    /// Get supported models
    fn supported_models(&self) -> Vec<String>;
}
