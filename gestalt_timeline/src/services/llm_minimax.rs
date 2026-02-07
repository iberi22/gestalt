//! LLM Service - MiniMax API Integration (Rust)
//!
//! Replaces AWS Bedrock/Claude with MiniMax API.
//! Uses the MiniMax provider from gestalt_core.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, error};
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

use crate::db::SurrealClient;
use crate::models::{EventType, Project, Task};
use crate::services::{Agent, TimelineService};
use gestalt_core::adapters::llm::minimax::MiniMaxProvider;
use gestalt_core::application::subagent::{SubagentRegistry, Subagent};

#[async_trait::async_trait]
pub trait Cognition: Send + Sync {
    async fn chat(&self, agent_id: &str, message: &str) -> Result<LLMResponse>;
    async fn orchestrate(&self, agent_id: &str, workflow_description: &str, project_context: Option<&str>) -> Result<Vec<OrchestrationAction>>;
    async fn orchestrate_step(&self, agent_id: &str, goal: &str, history: &[String], project_context: Option<&str>) -> Result<Vec<OrchestrationAction>>;
    fn model_id(&self) -> String;
    async fn list_models(&self) -> Result<Vec<String>>;
    async fn set_model(&self, model_id: &str) -> Result<()>;
}

/// LLM Service using MiniMax API (Rust implementation)
#[derive(Clone)]
pub struct LLMService {
    provider: Arc<MiniMaxProvider>,
    db: SurrealClient,
    timeline: TimelineService,
    model_id: Arc<RwLock<String>>,
    subagents: Arc<SubagentRegistry>,
    chat_history: Arc<Mutex<Vec<ChatMessage>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    role: String,
    content: String,
}

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

impl LLMService {
    /// Create a new LLMService with MiniMax provider.
    pub async fn new(
        db: SurrealClient,
        timeline: TimelineService,
        model_id: Option<String>,
    ) -> Result<Self> {
        info!("🤖 Initializing LLM Service with MiniMax...");

        let model = model_id.unwrap_or_else(|| {
            std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M2.1".to_string())
        });

        let provider = MiniMaxProvider::new(model.clone(), None);

        info!("📦 Using model: {}", model);

        Ok(Self {
            provider: Arc::new(provider),
            db,
            timeline,
            model_id: Arc::new(RwLock::new(model)),
            subagents: Arc::new(SubagentRegistry::new()),
            chat_history: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Send a chat message to MiniMax and get a response.
    pub async fn chat(&self, agent_id: &str, message: &str) -> Result<LLMResponse> {
        debug!("💬 Sending message to MiniMax: {}", message);
        let start = std::time::Instant::now();

        // Add user message to history
        let mut history = self.chat_history.lock().await;
        history.push(ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });

        // Detect subagent mention
        let (subagent, clean_message) = self.detect_subagent(message);
        let final_message = if let Some(agent) = &subagent {
            info!("🧙 Routing to subagent: {}", agent.name());
            format!("SYSTEM: {}\n\nUSER: {}", agent.system_prompt(), clean_message)
        } else {
            clean_message
        };

        // Build messages for API
        let messages: Vec<gestalt_core::ports::outbound::llm_provider::LlmMessage> = history
            .iter()
            .map(|m| gestalt_core::ports::outbound::llm_provider::LlmMessage {
                role: Some(m.role.clone()),
                content: m.content.clone(),
            })
            .collect();

        let request = gestalt_core::ports::outbound::llm_provider::LlmRequest {
            messages,
            model: self.model_id.read().unwrap().clone(),
            temperature: 0.7,
            max_tokens: 4096,
        };

        // Call MiniMax
        let response = self.provider.generate(request).await
            .context("Failed to call MiniMax API")?;

        // Add assistant response to history
        history.push(ChatMessage {
            role: "assistant".to_string(),
            content: response.content.clone(),
        });

        // Keep only last 20 messages
        if history.len() > 20 {
            let to_remove = history.len() - 20;
            history.drain(..to_remove);
        }

        let end = start.elapsed().as_millis() as u64;

        // Record in timeline
        self.timeline
            .emit(agent_id, EventType::Custom("llm_chat".to_string()))
            .await?;

        info!("✅ MiniMax response received ({} tokens in {}ms)", 
            response.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0), end);

        let usage = response.usage.unwrap_or_default();

        Ok(LLMResponse {
            content: response.content,
            model_id: self.model_id.read().unwrap().clone(),
            input_tokens: usage.prompt_tokens as i32,
            output_tokens: usage.completion_tokens as i32,
            duration_ms: end,
        })
    }

    /// Internal helper to detect subagent mention in prompt
    fn detect_subagent(&self, prompt: &str) -> (Option<Arc<dyn Subagent>>, String) {
        for word in prompt.split_whitespace() {
            if word.starts_with('@') {
                let name = &word[1..];
                if let Some(agent) = self.subagents.get(name) {
                    let clean_prompt = prompt.replace(word, "").trim().to_string();
                    return (Some(agent), clean_prompt);
                }
            }
        }
        (None, prompt.to_string())
    }

    /// Orchestrate a workflow based on natural language description.
    pub async fn orchestrate(
        &self,
        agent_id: &str,
        workflow_description: &str,
        project_context: Option<&str>,
    ) -> Result<Vec<OrchestrationAction>> {
        info!("🎯 Orchestrating workflow: {}", workflow_description);

        // Build context
        let system_prompt = self.build_orchestration_prompt(project_context).await?;
        let combined_message = format!("{}\n\nUSER REQUEST:\n{}", system_prompt, workflow_description);

        let messages = vec![
            gestalt_core::ports::outbound::llm_provider::LlmMessage {
                role: Some("system".to_string()),
                content: system_prompt,
            },
            gestalt_core::ports::outbound::llm_provider::LlmMessage {
                role: Some("user".to_string()),
                content: combined_message,
            },
        ];

        let request = gestalt_core::ports::outbound::llm_provider::LlmRequest {
            messages,
            model: self.model_id.read().unwrap().clone(),
            temperature: 0.3,
            max_tokens: 2048,
        };

        let start = std::time::Instant::now();
        let response = self.provider.generate(request).await
            .context("Failed to call MiniMax for orchestration")?;
        let end = start.elapsed().as_millis() as u64;

        info!("✅ MiniMax orchestration response in {}ms", end);

        let actions = self.parse_orchestration_response(&response.content)?;

        self.timeline
            .emit(agent_id, EventType::Custom("llm_orchestrate".to_string()))
            .await?;

        Ok(actions)
    }

    /// Execute a single step of the autonomous loop.
    pub async fn orchestrate_step(
        &self,
        agent_id: &str,
        goal: &str,
        history: &[String],
        project_context: Option<&str>,
    ) -> Result<Vec<OrchestrationAction>> {
        info!("🧠 Autonomous Step for Goal: {}", goal);

        let system_prompt = self.build_orchestration_prompt(project_context).await?;
        let history_text = if history.is_empty() {
            "(No history)".to_string()
        } else {
            history.join("\n")
        };

        let combined_msg = format!(
            "{}\n\nGOAL: {}\n\nHISTORY OF ACTIONS:\n{}\n\nINSTRUCTION: Review the history. If the goal is met, return empty list []. Otherwise, return the next JSON actions.",
            system_prompt, goal, history_text
        );

        let messages = vec![
            gestalt_core::ports::outbound::llm_provider::LlmMessage {
                role: Some("system".to_string()),
                content: system_prompt,
            },
            gestalt_core::ports::outbound::llm_provider::LlmMessage {
                role: Some("user".to_string()),
                content: combined_msg,
            },
        ];

        let request = gestalt_core::ports::outbound::llm_provider::LlmRequest {
            messages,
            model: self.model_id.read().unwrap().clone(),
            temperature: 0.3,
            max_tokens: 2048,
        };

        let start = std::time::Instant::now();
        let response = self.provider.generate(request).await
            .context("Failed to call MiniMax for step")?;
        let end = start.elapsed().as_millis() as u64;

        info!("✅ MiniMax step response in {}ms", end);

        self.parse_orchestration_response(&response.content)
    }

    /// Build the orchestration system prompt.
    async fn build_orchestration_prompt(&self, project_context: Option<&str>) -> Result<String> {
        let projects: Vec<Project> = self.db.select_all("projects").await.unwrap_or_default();
        let tasks: Vec<Task> = self.db.select_all("tasks").await.unwrap_or_default();

        let project_list = projects
            .iter()
            .map(|p| format!("- {} ({})", p.name, p.status))
            .collect::<Vec<_>>()
            .join("\n");

        let task_list = tasks
            .iter()
            .take(10)
            .map(|t| format!("- [{}] {}", t.status, t.description))
            .collect::<Vec<_>>()
            .join("\n");

        let context_section = if let Some(ctx) = project_context {
            format!("\nPROJECT CONTEXT: {}\n", ctx)
        } else {
            String::new()
        };

        Ok(format!(
            r#"You are a workflow orchestrator for Gestalt Timeline, a project and task management system.

CURRENT STATE:
Projects:
{}

Recent Tasks:
{}
{}

CAPABILITIES:
You can perform these actions by responding with JSON:
1. Create project: {{"action": "create_project", "name": "project-name", "description": "optional desc"}}
2. Create task: {{"action": "create_task", "project": "project-name", "description": "task description"}}
3. Run task: {{"action": "run_task", "task_id": "task-id"}}
4. List projects: {{"action": "list_projects"}}
5. List tasks: {{"action": "list_tasks", "project": "optional-project-name"}}
6. Get status: {{"action": "get_status", "project": "project-name"}}
7. Read file: {{"action": "read_file", "path": "path/to/file"}}
8. Write file: {{"action": "write_file", "path": "path/to/file", "content": "file content"}}
9. Execute shell: {{"action": "execute_shell", "command": "ls -la"}}
10. Start job: {{"action": "start_job", "name": "server", "command": "python3 -m http.server 8080"}}
11. Stop job: {{"action": "stop_job", "name": "server"}}
12. List jobs: {{"action": "list_jobs"}}
13. Delegate task: {{"action": "delegate_task", "agent": "<developer|researcher|reviewer>", "goal": "<goal>"}}
14. Call agent: {{"action": "call_agent", "tool": "<gh|aws|kubectl>", "args": ["arg1", "arg2"]}}
15. Await job: {{"action": "await_job", "job_id": "<job_id>"}}
Respond with a JSON array of actions."#,
            if project_list.is_empty() { "(none)" } else { &project_list },
            if task_list.is_empty() { "(none)" } else { &task_list },
            context_section
        ))
    }

    /// Parse the LLM response into orchestration actions.
    fn parse_orchestration_response(&self, response: &str) -> Result<Vec<OrchestrationAction>> {
        let json_str = if response.contains('[') {
            let start = response.find('[').unwrap();
            let end = response.rfind(']').map(|i| i + 1).unwrap_or(response.len());
            &response[start..end]
        } else if response.contains('{') {
            let start = response.find('{').unwrap();
            let end = response.rfind('}').map(|i| i + 1).unwrap_or(response.len());
            &format!("[{}]", &response[start..end])
        } else {
            return Ok(vec![OrchestrationAction::Chat {
                response: response.to_string(),
            }]);
        };

        let raw_actions: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .unwrap_or_else(|_| vec![serde_json::json!({"action": "chat", "response": response})]);

        let mut actions = Vec::new();

        for raw in raw_actions {
            let action_type = raw.get("action").and_then(|v| v.as_str()).unwrap_or("chat");

            let action = match action_type {
                "create_project" => OrchestrationAction::CreateProject {
                    name: raw.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed").to_string(),
                    description: raw.get("description").and_then(|v| v.as_str()).map(String::from),
                },
                "create_task" => OrchestrationAction::CreateTask {
                    project: raw.get("project").and_then(|v| v.as_str()).unwrap_or("default").to_string(),
                    description: raw.get("description").and_then(|v| v.as_str()).unwrap_or("task").to_string(),
                },
                "run_task" => OrchestrationAction::RunTask {
                    task_id: raw.get("task_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "list_projects" => OrchestrationAction::ListProjects,
                "list_tasks" => OrchestrationAction::ListTasks {
                    project: raw.get("project").and_then(|v| v.as_str()).map(String::from),
                },
                "get_status" => OrchestrationAction::GetStatus {
                    project: raw.get("project").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "read_file" => OrchestrationAction::ReadFile {
                    path: raw.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "write_file" => OrchestrationAction::WriteFile {
                    path: raw.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    content: raw.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "execute_shell" => OrchestrationAction::ExecuteShell {
                    command: raw.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "start_job" => OrchestrationAction::StartJob {
                    name: raw.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    command: raw.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "stop_job" => OrchestrationAction::StopJob {
                    name: raw.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                },
                "list_jobs" => OrchestrationAction::ListJobs,
                "delegate_task" => OrchestrationAction::DelegateTask {
                    agent: raw.get("agent").and_then(|v| v.as_str()).unwrap_or("general").to_string(),
                    goal: raw.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "call_agent" => OrchestrationAction::CallAgent {
                    tool: raw.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    args: raw.get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                },
                "await_job" => OrchestrationAction::AwaitJob {
                    job_id: raw.get("job_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                _ => OrchestrationAction::Chat {
                    response: raw.get("response").and_then(|v| v.as_str()).unwrap_or(response).to_string(),
                },
            };

            actions.push(action);
        }

        Ok(actions)
    }

    /// Get current model ID.
    pub fn get_model_id(&self) -> String {
        self.model_id.read().unwrap().clone()
    }

    /// Set the active model ID.
    pub fn set_model(&self, model_id: &str) -> Result<()> {
        let mut guard = self.model_id.write().unwrap();
        *guard = model_id.to_string();
        info!("🔄 Switched active model to: {}", model_id);
        Ok(())
    }

    /// List available models.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        Ok(self.provider.supported_models())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_orchestration_response_json_array() {
        let llm = LLMService::new_test().await;
        let response = r#"[{"action": "create_project", "name": "api", "description": "test"}]"#;
        let actions = llm.parse_orchestration_response(response).unwrap();
        assert_eq!(actions.len(), 1);
    }

    #[tokio::test]
    async fn test_parse_orchestration_response_chat() {
        let llm = LLMService::new_test().await;
        let response = "I don't understand";
        let actions = llm.parse_orchestration_response(response).unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OrchestrationAction::Chat { response: r } => assert!(r.contains("don't understand")),
            _ => panic!("Expected Chat"),
        }
    }
}

#[cfg(test)]
impl LLMService {
    async fn new_test() -> Self {
        // Create without database for testing
        let provider = MiniMaxProvider::new("test".to_string(), Some("test".to_string()));
        
        Self {
            provider: Arc::new(provider),
            db: SurrealClient::connect_test().await,
            timeline: TimelineService::new_test().await,
            model_id: Arc::new(RwLock::new("MiniMax-M2.1".to_string())),
            subagents: Arc::new(SubagentRegistry::new()),
            chat_history: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
