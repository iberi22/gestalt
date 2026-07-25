// ============================================================================
// OpenCode Go Provider
// ============================================================================
// Connects to https://opencode.ai/zen/go/v1 — OpenAI-compatible API.
// Uses OPENCODE_API_KEY env var (same as OPENCODE_GO_API_KEY).
// Supports all 14 models: deepseek-v4-flash, deepseek-v4-pro, kimi-k2.6,
// kimi-k2.5, glm-5, glm-5.1, minimax-m2.5, minimax-m2.7, qwen3.5-plus,
// qwen3.6-plus, mimo-v2-pro, mimo-v2-omni, mimo-v2.5, mimo-v2.5-pro

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OpenCodeGoProvider {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Option<Vec<Choice>>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: Option<String>,
    code: Option<String>,
}

impl OpenCodeGoProvider {
    /// Create a new OpenCode Go provider with a specific model
    pub fn new(model: &str, api_key: &str) -> Self {
        Self {
            endpoint: "https://opencode.ai/zen/go/v1/chat/completions".to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    /// Create from environment variable with default model
    pub fn from_env(model: &str) -> Result<Self> {
        let api_key = std::env::var("OPENCODE_API_KEY")
            .or_else(|_| std::env::var("OPENCODE_GO_API_KEY"))
            .map_err(|_| anyhow::anyhow!(
                "OPENCODE_API_KEY or OPENCODE_GO_API_KEY is required for OpenCode Go provider"
            ))?;

        Ok(Self::new(model, &api_key))
    }

    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a helpful SWAL coding agent."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.3,
            "max_tokens": 4096
        });

        let response = self.client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("OpenCode Go request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OpenCode Go API error ({}): {}", status, error_text));
        }

        let resp: OpenAIResponse = response.json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse OpenCode Go response: {}", e))?;

        if let Some(error) = resp.error {
            return Err(anyhow::anyhow!("OpenCode Go API error: {} ({})",
                error.message.unwrap_or_default(),
                error.code.unwrap_or_default()
            ));
        }

        let text = resp.choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .ok_or_else(|| anyhow::anyhow!("No text in OpenCode Go response"))?;

        Ok(text)
    }

    pub fn name(&self) -> &str {
        &self.model
    }
}
