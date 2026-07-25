//! Xavier2 Agent Implementation
//! 
//! Implements GraphNode for integration with Gestalt Swarm.

use serde::{Serialize, Deserialize};
use super::client::{MemoryResult, Xavier2Client};
use crate::application::agent::tools::create_gestalt_tools;
use serde_json::json;
use std::sync::Arc;
use synapse_agentic::framework::workflow::*;

/// Task actions for Xavier2
#[derive(Debug, Clone)]
pub enum Xavier2Action {
    SearchMemory,
    AddMemory,
    GetContext,
    GetProjectState,
    SaveProjectState,
}

/// A task for Xavier2 agent
#[derive(Debug, Clone)]
pub struct Xavier2Task {
    pub action: Xavier2Action,
    pub query: Option<String>,
    pub content: Option<String>,
    pub path: Option<String>,
    pub max_results: usize,
}

impl Xavier2Task {
    pub fn search(query: impl Into<String>, max_results: usize) -> Self {
        Self {
            action: Xavier2Action::SearchMemory,
            query: Some(query.into()),
            content: None,
            path: None,
            max_results,
        }
    }

    pub fn add(content: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            action: Xavier2Action::AddMemory,
            query: None,
            content: Some(content.into()),
            path: Some(path.into()),
            max_results: 10,
        }
    }
}

/// Xavier2 Agent for Gestalt Swarm
/// 
/// Acts as a subagent that handles:
/// - Memory search and retrieval
/// - Project context management
/// - Fast embeddings lookup
pub struct Xavier2Agent {
    client: Xavier2Client,
    node_id: String,
}

impl Xavier2Agent {
    /// Create new Xavier2 agent
    pub async fn new(endpoint: impl Into<String>, token: impl Into<String>) -> anyhow::Result<Self> {
        let client = Xavier2Client::new(endpoint.into(), token.into());
        
        // Verify connectivity
        if !client.is_available().await {
            anyhow::bail!("Xavier2 is not available at the specified endpoint");
        }
        
        Ok(Self {
            client,
            node_id: format!("xavier2-{}", uuid::Uuid::new_v4()),
        })
    }

    /// Create from environment variables
    pub async fn from_env() -> anyhow::Result<Self> {
        Self::new(
            std::env::var("XAVIER2_URL").unwrap_or_else(|_| "http://localhost:8006".into()),
            std::env::var("XAVIER2_TOKEN").unwrap_or_else(|_| "dev-token".into()),
        ).await
    }

    /// Execute a search query
    pub async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<MemoryResult>> {
        let response = self.client.search(query, max_results).await?;
        Ok(response.results)
    }

    /// Add a memory entry
    pub async fn add_memory(&self, content: &str, path: &str) -> anyhow::Result<String> {
        let response = self.client.add(content, path, json!({})).await?;
        Ok(response.id)
    }

    /// Get project context
    pub async fn get_context(&self, project: &str) -> anyhow::Result<serde_json::Value> {
        let results = self.client.search(
            &format!("project {} context state", project),
            5,
        ).await?;
        
        let context: Vec<_> = results.results.into_iter().map(|r| json!({
            "path": r.path,
            "content": r.content,
            "metadata": r.metadata,
        })).collect();
        
        Ok(json!({
            "project": project,
            "context": context,
            "count": context.len(),
        }))
    }

    /// Save project state
    pub async fn save_project_state(
        &self,
        project: &str,
        state: &str,
    ) -> anyhow::Result<String> {
        let path = format!("projects/{}/state", project);
        self.add_memory(state, &path).await
    }
}

impl std::fmt::Debug for Xavier2Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xavier2Agent")
            .field("endpoint", &self.client.endpoint)
            .field("node_id", &self.node_id)
            .finish()
    }
}

impl Clone for Xavier2Agent {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            node_id: self.node_id.clone(),
        }
    }
}

impl Xavier2Agent {
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

// =============================================================================
// Swarm Coordinator Integration
// =============================================================================

/// Message for swarm communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Xavier2Message {
    #[serde(rename = "search")]
    Search {
        query: String,
        max_results: Option<usize>,
    },
    #[serde(rename = "add")]
    Add {
        content: String,
        path: String,
    },
    #[serde(rename = "context")]
    Context { project: String },
    #[serde(rename = "save_state")]
    SaveState {
        project: String,
        state: String,
    },
}

/// Response from Xavier2
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Xavier2Response {
    #[serde(rename = "search_result")]
    SearchResult {
        count: usize,
        results: Vec<MemoryResult>,
    },
    #[serde(rename = "add_result")]
    AddResult { id: String },
    #[serde(rename = "context_result")]
    ContextResult { context: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
}

impl Xavier2Agent {
    /// Handle a message from the swarm
    pub async fn handle_message(&self, msg: Xavier2Message) -> Xavier2Response {
        match msg {
            Xavier2Message::Search { query, max_results } => {
                match self.search(&query, max_results.unwrap_or(10)).await {
                    Ok(results) => Xavier2Response::SearchResult {
                        count: results.len(),
                        results,
                    },
                    Err(e) => Xavier2Response::Error {
                        message: e.to_string(),
                    },
                }
            }
            Xavier2Message::Add { content, path } => {
                match self.add_memory(&content, &path).await {
                    Ok(id) => Xavier2Response::AddResult { id },
                    Err(e) => Xavier2Response::Error {
                        message: e.to_string(),
                    },
                }
            }
            Xavier2Message::Context { project } => {
                match self.get_context(&project).await {
                    Ok(context) => Xavier2Response::ContextResult { context },
                    Err(e) => Xavier2Response::Error {
                        message: e.to_string(),
                    },
                }
            }
            Xavier2Message::SaveState { project, state } => {
                match self.save_project_state(&project, &state).await {
                    Ok(id) => Xavier2Response::AddResult { id },
                    Err(e) => Xavier2Response::Error {
                        message: e.to_string(),
                    },
                }
            }
        }
    }
}

// =============================================================================
// Re-exports
// =============================================================================

// Note: Xavier2Agent, Xavier2Action, Xavier2Task, Xavier2Message, Xavier2Response
// are already defined in this module and don't need re-export.
// The mod.rs exports them correctly.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Xavier2Task::search("test query", 5);
        assert!(matches!(task.action, Xavier2Action::SearchMemory));
        assert_eq!(task.query.as_deref(), Some("test query"));
        assert_eq!(task.max_results, 5);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Xavier2Message::Search {
            query: "test".into(),
            max_results: Some(10),
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("search"));
        assert!(json.contains("test"));
    }
}
