use async_trait::async_trait;
use serde::Deserialize;

#[async_trait]
pub trait LLMProvider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn cost_per_1k_tokens(&self) -> f64;
    async fn generate(&self, prompt: &str) -> anyhow::Result<String>;
}

pub trait Provider: LLMProvider {}
impl<T: LLMProvider> Provider for T {}

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    api_key: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

#[async_trait]
impl LLMProvider for GeminiProvider {
    fn name(&self) -> &str {
        &self.model
    }
    fn cost_per_1k_tokens(&self) -> f64 {
        0.0
    }
    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .or_else(|_| Ok(self.api_key.clone()))
            .map_err(|_: std::env::VarError| anyhow::anyhow!("GEMINI_API_KEY not set"))?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );

        let body = serde_json::json!({
            "contents": [{"parts": [{"text": prompt}]}],
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 2048
            }
        });

        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Gemini API request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Gemini API error ({}): {}", status, error_text);
            return Err(anyhow::anyhow!("Gemini API returned error: {}", status));
        }

        #[derive(Deserialize)]
        struct GeminiResponse {
            candidates: Option<Vec<Candidate>>,
        }
        #[derive(Deserialize)]
        struct Candidate {
            content: Option<Content>,
        }
        #[derive(Deserialize)]
        struct Content {
            parts: Option<Vec<Part>>,
        }
        #[derive(Deserialize)]
        struct Part {
            text: Option<String>,
        }

        let resp: GeminiResponse = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Gemini response: {}", e))?;

        let text = resp
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|p| p.into_iter().next())
            .and_then(|p| p.text)
            .ok_or_else(|| anyhow::anyhow!("No text in Gemini response"))?;

        Ok(text)
    }
}

#[derive(Debug, Clone)]
pub struct GroqProvider {
    api_key: String,
    model: String,
}

impl GroqProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

#[async_trait]
impl LLMProvider for GroqProvider {
    fn name(&self) -> &str {
        &self.model
    }
    fn cost_per_1k_tokens(&self) -> f64 {
        0.0
    }
    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        let api_key = std::env::var("GROQ_API_KEY")
            .or_else(|_| Ok(self.api_key.clone()))
            .map_err(|_: std::env::VarError| anyhow::anyhow!("GROQ_API_KEY not set"))?;

        let client = reqwest::Client::new();
        let url = "https://api.groq.com/openai/v1/chat/completions";

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.7,
            "max_tokens": 2048
        });

        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Groq API request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Groq API error ({}): {}", status, error_text);
            return Err(anyhow::anyhow!("Groq API returned error: {}", status));
        }

        #[derive(Deserialize)]
        struct GroqResponse {
            choices: Option<Vec<Choice>>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Option<MessageDetail>,
        }
        #[derive(Deserialize)]
        struct MessageDetail {
            content: Option<String>,
        }

        let resp: GroqResponse = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Groq response: {}", e))?;

        let text = resp
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .ok_or_else(|| anyhow::anyhow!("No text in Groq response"))?;

        Ok(text)
    }
}

#[derive(Debug, Clone)]
pub struct MinimaxProvider {
    api_key: String,
    group_id: String,
    model: String,
}

impl MinimaxProvider {
    pub fn new(api_key: String, group_id: String, model: String) -> Self {
        Self {
            api_key,
            group_id,
            model,
        }
    }
}

#[async_trait]
impl LLMProvider for MinimaxProvider {
    fn name(&self) -> &str {
        &self.model
    }
    fn cost_per_1k_tokens(&self) -> f64 {
        0.0
    }
    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        let api_key = std::env::var("MINIMAX_API_KEY")
            .or_else(|_| Ok(self.api_key.clone()))
            .map_err(|_: std::env::VarError| anyhow::anyhow!("MINIMAX_API_KEY not set"))?;

        let group_id = std::env::var("MINIMAX_GROUP_ID")
            .or_else(|_| Ok(self.group_id.clone()))
            .map_err(|_: std::env::VarError| anyhow::anyhow!("MINIMAX_GROUP_ID not set"))?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://api.minimax.chat/v1/text/chatcompletion_pro?GroupId={}",
            group_id
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.7,
            "max_tokens": 2048
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("MiniMax API request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("MiniMax API error ({}): {}", status, error_text);
            return Err(anyhow::anyhow!("MiniMax API returned error: {}", status));
        }

        #[derive(Deserialize)]
        struct MinimaxResponse {
            choices: Option<Vec<Choice>>,
        }
        #[derive(Deserialize)]
        struct Choice {
            messages: Option<Vec<Message>>,
        }
        #[derive(Deserialize)]
        struct Message {
            text: Option<String>,
        }

        let resp: MinimaxResponse = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse MiniMax response: {}", e))?;

        let text = resp
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.messages)
            .and_then(|m| m.into_iter().next())
            .and_then(|m| m.text)
            .ok_or_else(|| anyhow::anyhow!("No text in MiniMax response"))?;

        Ok(text)
    }
}
