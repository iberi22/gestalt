//! Xavier Agent Implementation
//!
//! Basic stub for Xavier v0.12.0 integration with Gestalt Swarm.

use super::client::{MemoryResult, XavierClient};
use serde_json::json;

/// Xavier Agent for Gestalt Swarm
///
/// Acts as a subagent that handles memory search and retrieval
/// via the Xavier v0.12.0 API.
pub struct XavierAgent {
    client: XavierClient,
    node_id: String,
}

impl XavierAgent {
    /// Create new Xavier agent
    pub async fn new(
        endpoint: impl Into<String>,
        token: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let client = XavierClient::new(endpoint.into(), token.into())
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        if !client.is_available().await {
            anyhow::bail!("Xavier is not available at the specified endpoint");
        }

        Ok(Self {
            client,
            node_id: format!("xavier-{}", uuid::Uuid::new_v4()),
        })
    }

    /// Create from environment variables
    pub async fn from_env() -> anyhow::Result<Self> {
        Self::new(
            std::env::var("XAVIER_URL").unwrap_or_else(|_| "http://localhost:8006".into()),
            std::env::var("XAVIER_TOKEN").unwrap_or_default(),
        )
        .await
    }

    /// Execute a search query
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        mode: &str,
    ) -> anyhow::Result<Vec<MemoryResult>> {
        let response = self.client.search(query, limit, mode).await?;
        Ok(response.results)
    }

    /// Add a memory entry
    pub async fn add_memory(
        &self,
        content: &str,
        path: &str,
        kind: &str,
    ) -> anyhow::Result<String> {
        let response = self.client.add(content, path, kind, json!({})).await?;
        Ok(response.id)
    }

    /// Archive a decision memory
    pub async fn archive_decision(
        &self,
        decision_text: &str,
        metadata: serde_json::Value,
    ) -> anyhow::Result<String> {
        self.client.archive_decision(decision_text, metadata).await
    }

    /// Get the node ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Get the endpoint
    pub fn endpoint(&self) -> &str {
        &self.client.endpoint
    }

    /// Health check
    pub async fn health_check(&self) -> bool {
        self.client.is_available().await
    }
}

impl std::fmt::Debug for XavierAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XavierAgent")
            .field("endpoint", &self.client.endpoint)
            .field("node_id", &self.node_id)
            .finish()
    }
}

impl Clone for XavierAgent {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            node_id: self.node_id.clone(),
        }
    }
}
