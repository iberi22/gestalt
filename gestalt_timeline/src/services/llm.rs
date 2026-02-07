//! LLM Services Module
//!
//! Provides trait definitions for LLM services.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

/// LLM Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: String,
    pub model_id: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub duration_ms: u64,
}

/// Orchestration action parsed from LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestrationAction {
    CreateProject { name: String, description: Option<String> },
    CreateTask { project: String, description: String },
    RunTask { task_id: String },
    ListProjects,
    ListTasks { project: Option<String> },
    GetStatus { project: String },
    Chat { response: String },
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    ExecuteShell { command: String },
    StartJob { name: String, command: String },
    StopJob { name: String },
    ListJobs,
    DelegateTask { agent: String, goal: String },
    CallAgent { tool: String, args: Vec<String> },
    AwaitJob { job_id: String },
}

/// Cognition trait for LLM services
#[async_trait]
pub trait Cognition: Send + Sync {
    /// Send a chat message and get a response
    async fn chat(&self, agent_id: &str, message: &str) -> Result<LLMResponse, anyhow::Error>;
    
    /// Orchestrate a workflow
    async fn orchestrate(&self, agent_id: &str, workflow_description: &str, project_context: Option<&str>) -> Result<Vec<OrchestrationAction>, anyhow::Error>;
    
    /// Execute a single step of the autonomous loop
    async fn orchestrate_step(&self, agent_id: &str, goal: &str, history: &[String], project_context: Option<&str>) -> Result<Vec<OrchestrationAction>, anyhow::Error>;
    
    /// Get the current model ID
    fn model_id(&self) -> String;
    
    /// List available models
    async fn list_models(&self) -> Result<Vec<String>, anyhow::Error>;
    
    /// Set the active model
    async fn set_model(&self, model_id: &str) -> Result<(), anyhow::Error>;
}
