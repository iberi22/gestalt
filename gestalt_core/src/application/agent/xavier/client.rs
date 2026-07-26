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
    pub fn new(endpoint: String, token: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            endpoint,
            token,
            client,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("XAVIER_URL").unwrap_or_else(|_| "http://localhost:8006".into()),
            std::env::var("XAVIER_TOKEN")
                .unwrap_or_else(|_| "mZHbmzjEKrBohyzkWtVkKemWdYytuFEP".into()),
        )
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = XavierClient::new("http://localhost:8006".into(), "test-token".into());

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
