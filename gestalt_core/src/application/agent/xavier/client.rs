//! Xavier HTTP Client
//!
//! Lightweight client for Xavier v0.12.0 Memory API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct XavierClient {
    pub endpoint: String,
    pub token: String,
    client: Client,
}

#[derive(Debug, Serialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: usize,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub count: usize,
    pub results: Vec<MemoryResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResult {
    pub id: String,
    pub path: String,
    pub content: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Serialize)]
pub struct AddMemoryRequest {
    pub content: String,
    pub path: String,
    pub kind: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AddMemoryResponse {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct StatsResponse {
    pub total_pages: usize,
    pub total_memories: usize,
    pub storage_bytes: usize,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

impl XavierClient {
    /// Create new Xavier client
    pub fn new(endpoint: String, token: String) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            endpoint,
            token,
            client,
        })
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("XAVIER_URL").unwrap_or_else(|_| "http://localhost:8006".into()),
            std::env::var("XAVIER_TOKEN").unwrap_or_default(),
        )
        .expect("XavierClient::from_env() failed — check TLS/network configuration")
    }

    /// Search memory
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        mode: &str,
    ) -> anyhow::Result<SearchResponse> {
        let req = SearchRequest {
            query: query.into(),
            limit,
            mode: mode.into(),
        };

        let resp = self
            .client
            .post(format!("{}/v1/memories/search", self.endpoint))
            .header("X-Xavier-Token", &self.token)
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Xavier search failed: {}", resp.status());
        }

        Ok(resp.json().await?)
    }

    /// Add memory
    pub async fn add(
        &self,
        content: &str,
        path: &str,
        kind: &str,
        metadata: serde_json::Value,
    ) -> anyhow::Result<AddMemoryResponse> {
        let req = AddMemoryRequest {
            content: content.into(),
            path: path.into(),
            kind: kind.into(),
            metadata,
        };

        let resp = self
            .client
            .post(format!("{}/v1/memories", self.endpoint))
            .header("X-Xavier-Token", &self.token)
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Xavier add failed: {}", resp.status());
        }

        Ok(resp.json().await?)
    }

    /// Get stats
    pub async fn stats(&self) -> anyhow::Result<StatsResponse> {
        let resp = self
            .client
            .get(format!("{}/v1/stats", self.endpoint))
            .header("X-Xavier-Token", &self.token)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Xavier stats failed: {}", resp.status());
        }

        Ok(resp.json().await?)
    }

    /// Health check
    pub async fn health(&self) -> anyhow::Result<HealthResponse> {
        let resp = self
            .client
            .get(format!("{}/health", self.endpoint))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Xavier health failed: {}", resp.status());
        }

        Ok(resp.json().await?)
    }

    /// Check if Xavier is available
    pub async fn is_available(&self) -> bool {
        self.health().await.is_ok()
    }

    /// Search context — convenience wrapper around `search` that returns
    /// a flat `Vec<String>` of the top-N snippet results.
    ///
    /// Uses hybrid search mode by default. Returns an empty vec on failure
    /// (non-fatal; Xavier may be unavailable).
    pub async fn search_context(&self, query: &str, limit: usize) -> Vec<String> {
        match self.search(query, limit, "hybrid").await {
            Ok(resp) => resp
                .results
                .into_iter()
                .map(|r| {
                    if r.snippet.is_empty() {
                        r.content
                    } else {
                        r.snippet
                    }
                })
                .collect(),
            Err(e) => {
                tracing::warn!("Xavier search_context failed (non-fatal): {e}");
                Vec::new()
            }
        }
    }

    /// Archive a completed run result as a Xavier memory.
    ///
    /// Stores the serialized content at `gestalt/run/{run_id}` with
    /// kind=`run_result`. Returns the assigned memory ID on success.
    pub async fn archive_run(
        &self,
        content: &str,
        run_id: &str,
        metadata: serde_json::Value,
    ) -> anyhow::Result<String> {
        let path = format!("gestalt/run/{run_id}");
        let resp = self.add(content, &path, "run_result", metadata).await?;
        Ok(resp.id)
    }

    /// Index a plan document in Xavier as kind=plan
    pub async fn save_plan(&self, content: &str, path: &str) -> Result<String, String> {
        self.add_memory(content, path, "plan").await
    }

    /// Index an execution result as kind=execution
    pub async fn save_execution(&self, content: &str, path: &str) -> Result<String, String> {
        self.add_memory(content, path, "execution").await
    }

    /// Index a config document as kind=config
    pub async fn save_config(&self, content: &str, path: &str) -> Result<String, String> {
        self.add_memory(content, path, "config").await
    }

    /// Generic memory add with kind
    pub async fn add_memory(&self, content: &str, path: &str, kind: &str) -> Result<String, String> {
        let url = format!("{}/v1/memories", self.endpoint);
        let body = serde_json::json!({
            "content": content,
            "path": path,
            "kind": kind,
        });
        let resp = self
            .client
            .post(&url)
            .header("X-Xavier-Token", &self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Xavier request failed: {}", e))?;
        let data: serde_json::Value = resp.json().await
            .map_err(|e| format!("Xavier response parse failed: {}", e))?;
        Ok(data["id"].as_str().unwrap_or("ok").to_string())
    }

    /// Health check: returns status string
    pub async fn health_check(&self) -> Result<String, String> {
        let url = format!("{}/health", self.endpoint);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;
        let data: serde_json::Value = resp.json().await
            .map_err(|e| format!("Health response parse failed: {}", e))?;
        Ok(data["status"].as_str().unwrap_or("unknown").to_string())
    }

    /// Check if embeddings are functional
    pub async fn embedding_status(&self) -> Result<String, String> {
        let url = format!("{}/health", self.endpoint);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;
        let data: serde_json::Value = resp.json().await
            .map_err(|e| format!("Health response parse failed: {}", e))?;
        let mode = data["mode"].as_str().unwrap_or("unknown");
        Ok(mode.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = XavierClient::new("http://localhost:8006".into(), "test-token".into()).unwrap();

        assert_eq!(client.endpoint, "http://localhost:8006");
        assert_eq!(client.token, "test-token");
    }

    #[test]
    fn test_search_request_serialization() {
        let req = SearchRequest {
            query: "test query".into(),
            limit: 5,
            mode: "hybrid".into(),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test query"));
        assert!(json.contains("5"));
        assert!(json.contains("hybrid"));
    }
}
