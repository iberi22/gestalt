//! OpenClaw ↔ Synapse Agentic Bridge
//! 
//! This module provides integration between Gestalt MCP and the Synapse Agentic framework.
//! It allows OpenClaw to use agents, skills, and the event bus from Rust.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

fn current_time() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", now)
}

/// Message types for the bridge
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeMessage {
    /// Execute a task
    Execute { task_id: String, command: String },
    /// Query status
    Query { query: String },
    /// Event from agentic system
    Event { event_type: String, data: serde_json::Value },
    /// Response
    Response { request_id: String, result: serde_json::Value },
}

/// Bridge state
#[derive(Clone, Debug)]
pub struct BridgeState {
    pub is_connected: bool,
    pub last_ping: Option<std::time::Instant>,
    pub active_tasks: Arc<RwLock<std::collections::HashMap<String, TaskState>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub result: Option<String>,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            is_connected: false,
            last_ping: None,
            active_tasks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl BridgeState {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub async fn add_task(&self, task_id: String) {
        let task_id_clone = task_id.clone();
        let mut tasks = self.active_tasks.write().await;
        tasks.insert(task_id, TaskState {
            id: task_id_clone,
            status: "pending".to_string(),
            created_at: current_time(),
            result: None,
        });
    }
    
    pub async fn update_task(&self, task_id: &str, status: &str, result: Option<String>) {
        let mut tasks = self.active_tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status.to_string();
            task.result = result;
        }
    }
    
    pub async fn get_task(&self, task_id: &str) -> Option<TaskState> {
        let tasks = self.active_tasks.read().await;
        tasks.get(task_id).cloned()
    }
    
    pub async fn list_tasks(&self) -> Vec<TaskState> {
        let tasks = self.active_tasks.read().await;
        tasks.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_bridge_state() {
        let state = BridgeState::new();
        
        state.add_task("test_1".to_string()).await;
        
        let tasks = state.list_tasks().await;
        assert_eq!(tasks.len(), 1);
        
        state.update_task("test_1", "completed", Some("success".to_string())).await;
        
        let task = state.get_task("test_1").await;
        assert!(task.is_some());
        assert_eq!(task.unwrap().status, "completed");
    }
}
