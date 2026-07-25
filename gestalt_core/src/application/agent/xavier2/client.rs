//! Xavier2 HTTP Client
//! 
//! Lightweight client for Xavier2 Memory API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Xavier2Client {
    pub endpoint: String,
    pub token: String,
    client: Client,
}

#[derive(Debug, Serialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

fn default_max_results() -> usize {
    10
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub count: usize,
    pub query: String,
    pub results: Vec<MemoryResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResult {
    pub id: String,
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AddMemoryRequest {
    pub content: String,
    pub path: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AddMemoryResponse {
    pub id: String,
    pub path: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct StatsResponse {
    pub total_memories: usize,
    pub storage_bytes: usize,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

impl Xavier2Client {
    /// Create new Xavier2 client
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
            std::env::var("XAVIER2_URL")
                .unwrap_or_else(|_| "http://localhost:8006".into()),
            std::env::var("XAVIER2_TOKEN")
                .unwrap_or_else(|_| "dev-token".into()),
        )
    }

    /// Search memory
    pub async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<SearchResponse> {
        let req = SearchRequest {
            query: query.into(),
            max_results,
        };
        
        let resp = self.client
            .post(format!("{}/memory/search", self.endpoint))
            .header("X-Cortex-Token", &self.token)
            .json(&req)
            .send()
            .await?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Xavier2 search failed: {}", resp.status());
        }
        
        Ok(resp.json().await?)
    }

    /// Add memory
    pub async fn add(&self, content: &str, path: &str, metadata: serde_json::Value) -> anyhow::Result<AddMemoryResponse> {
        let req = AddMemoryRequest {
            content: content.into(),
            path: path.into(),
            metadata,
        };
        
        let resp = self.client
            .post(format!("{}/memory/add", self.endpoint))
            .header("X-Cortex-Token", &self.token)
            .json(&req)
            .send()
            .await?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Xavier2 add failed: {}", resp.status());
        }
        
        Ok(resp.json().await?)
    }

    /// Get stats
    pub async fn stats(&self) -> anyhow::Result<StatsResponse> {
        let resp = self.client
            .get(format!("{}/memory/stats", self.endpoint))
            .header("X-Cortex-Token", &self.token)
            .send()
            .await?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Xavier2 stats failed: {}", resp.status());
        }
        
        Ok(resp.json().await?)
    }

    /// Health check
    pub async fn health(&self) -> anyhow::Result<HealthResponse> {
        let resp = self.client
            .get(format!("{}/health", self.endpoint))
            .send()
            .await?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Xavier2 health failed: {}", resp.status());
        }
        
        Ok(resp.json().await?)
    }

    /// Check if Xavier2 is available
    pub async fn is_available(&self) -> bool {
        self.health().await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Xavier2Client::new(
            "http://localhost:8006".into(),
            "test-token".into(),
        );
        
        assert_eq!(client.endpoint, "http://localhost:8006");
        assert_eq!(client.token, "test-token");
    }

    #[test]
    fn test_search_request_serialization() {
        let req = SearchRequest {
            query: "test query".into(),
            max_results: 5,
        };
        
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test query"));
        assert!(json.contains("5"));
    }
}
