// ============================================================================
// OpenAI-Compatible Provider (NVIDIA, Groq, OpenRouter)
// ============================================================================

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OpenAiCompatProvider {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub provider_name: String,
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

impl OpenAiCompatProvider {
    pub fn nvidia(model: &str, api_key: &str) -> Self {
        Self {
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            model: if model.contains("devstral") {
                "meta/llama-3.3-70b-instruct".to_string()
            } else {
                model.to_string()
            },
            api_key: api_key.to_string(),
            provider_name: "NVIDIA".to_string(),
            client: Client::new(),
        }
    }

    pub fn groq(model: &str, api_key: &str) -> Self {
        Self {
            endpoint: "https://api.groq.com/openai/v1/chat/completions".to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            provider_name: "Groq".to_string(),
            client: Client::new(),
        }
    }

    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a helpful SWAL coding agent."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.3,
            "max_tokens": 2048
        });

        let response = self.client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("{} request failed: {}", self.provider_name, e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("{} API error ({}): {}", self.provider_name, status, error_text));
        }

        let resp: OpenAIResponse = response.json().await
            .map_err(|e| anyhow::anyhow!("Failed to parse {} response: {}", self.provider_name, e))?;

        if let Some(error) = resp.error {
            return Err(anyhow::anyhow!("{} API error: {} ({})", 
                self.provider_name, 
                error.message.unwrap_or_default(),
                error.code.unwrap_or_default()
            ));
        }

        let text = resp.choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .ok_or_else(|| anyhow::anyhow!("No text in {} response", self.provider_name))?;

        Ok(text)
    }

    pub fn name(&self) -> &str {
        &self.provider_name
    }
}
