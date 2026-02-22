use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::services::{AgentService, ProjectService, TaskService, WatchService};
use synapse_agentic::prelude::{DecisionContext, DecisionEngine, EmptyContext, ToolRegistry};

/// Orchestration action executed by AgentRuntime.
#[derive(Debug, Clone)]
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
    CallAgent { tool: String, args: Value },
    AwaitJob { job_id: String },
}

/// The Autonomous Agent Runtime.
/// Encapsulates the "Think-Act-Observe" loop.
#[derive(Clone)]
pub struct AgentRuntime {
    agent_id: String,
    engine: Arc<DecisionEngine>,
    registry: Arc<ToolRegistry>,
    project: ProjectService,
    task: TaskService,
    watch: WatchService,
    agent: AgentService,
    max_steps: usize,
    jobs: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
}

impl AgentRuntime {
    pub fn new(
        agent_id: String,
        engine: Arc<DecisionEngine>,
        registry: Arc<ToolRegistry>,
        project: ProjectService,
        task: TaskService,
        watch: WatchService,
        agent: AgentService,
    ) -> Self {
        Self {
            agent_id,
            engine,
            registry,
            project,
            task,
            watch,
            agent,
            max_steps: 20,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Run the autonomous loop for a specific goal.
    pub async fn run_loop(&self, goal: &str) -> Result<()> {
        info!("Starting Autonomous Loop for Agent: {}", self.agent_id);
        info!("Goal: {}", goal);

        use crate::services::AgentStatus;
        let _ = self.agent.set_status(&self.agent_id, AgentStatus::Busy).await;

        let mut conversation_history = Vec::new();
        conversation_history.push(format!("GOAL: {}\n\nPlease start working on this goal.", goal));

        for step in 0..self.max_steps {
            info!("Step {}/{}", step + 1, self.max_steps);
            let actions = self.next_actions(goal, &conversation_history).await?;

            if actions.is_empty() {
                info!("Agent decided to stop (no actions).");
                break;
            }

            for action in actions {
                conversation_history.push(format!("Action: {:?}", action));
                let observation = match self.execute_action(&action).await {
                    Ok(obs) => obs,
                    Err(e) => format!("Error: {}", e),
                };
                conversation_history.push(format!("Observation: {}", observation));
            }
        }

        let _ = self.agent.set_status(&self.agent_id, AgentStatus::Idle).await;
        Ok(())
    }

    async fn next_actions(&self, goal: &str, history: &[String]) -> Result<Vec<OrchestrationAction>> {
        let history_summary = history
            .iter()
            .rev()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        let context = DecisionContext::new("orchestration")
            .with_summary(format!("goal: {}\n\nhistory:\n{}", goal, history_summary));
        let decision = self.engine.decide(&context).await?;
        Ok(Self::map_decision_to_actions(&decision.action, &decision.reasoning))
    }

    fn map_decision_to_actions(action: &str, reasoning: &str) -> Vec<OrchestrationAction> {
        match action.trim().to_lowercase().as_str() {
            "stop" | "done" | "defer" => vec![],
            "list_projects" => vec![OrchestrationAction::ListProjects],
            "list_jobs" => vec![OrchestrationAction::ListJobs],
            "chat" => vec![OrchestrationAction::Chat {
                response: reasoning.to_string(),
            }],
            _ => vec![OrchestrationAction::Chat {
                response: format!("Decision: {} | {}", action, reasoning),
            }],
        }
    }

    async fn execute_action(&self, action: &OrchestrationAction) -> Result<String> {
        match action {
            OrchestrationAction::CreateProject { name, description: _description } => {
                self.project.create_project(name, &self.agent_id).await?;
                Ok(format!("Created Project '{}'", name))
            }
            OrchestrationAction::CreateTask { project, description } => {
                self.task.create_task(project, description, &self.agent_id).await?;
                Ok(format!("Created Task in '{}'", project))
            }
            OrchestrationAction::RunTask { task_id } => {
                self.task.run_task(task_id, &self.agent_id).await?;
                Ok(format!("Ran Task '{}'", task_id))
            }
            OrchestrationAction::ListProjects => {
                let projects = self.project.list_projects().await?;
                Ok(format!("Listed {} Projects", projects.len()))
            }
            OrchestrationAction::ListTasks { project } => {
                let tasks = self.task.list_tasks(project.as_deref()).await?;
                Ok(format!("Listed {} Tasks", tasks.len()))
            }
            OrchestrationAction::GetStatus { project } => {
                let status = self.project.get_status(project).await?;
                Ok(format!("Project '{}' is {}% complete", project, status.progress_percent))
            }
            OrchestrationAction::Chat { response } => {
                info!("Agent says: {}", response);
                Ok(format!("Agent said: {}", response))
            }
            OrchestrationAction::ReadFile { path } => {
                match tokio::fs::read_to_string(path).await {
                    Ok(content) => Ok(format!("File '{}' content:\n{}", path, content)),
                    Err(e) => Ok(format!("Error reading file '{}': {}", path, e)),
                }
            }
            OrchestrationAction::WriteFile { path, content } => {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                match tokio::fs::write(path, content).await {
                    Ok(_) => Ok(format!("Successfully wrote to file '{}'", path)),
                    Err(e) => Ok(format!("Error writing file '{}': {}", path, e)),
                }
            }
            OrchestrationAction::ExecuteShell { command } => {
                #[cfg(target_os = "windows")]
                let mut cmd = tokio::process::Command::new("powershell");
                #[cfg(target_os = "windows")]
                cmd.arg("-Command").arg(command);

                #[cfg(not(target_os = "windows"))]
                let mut cmd = tokio::process::Command::new("sh");
                #[cfg(not(target_os = "windows"))]
                cmd.arg("-c").arg(command);

                match cmd.output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let exit_code = output.status.code().unwrap_or(-1);
                        Ok(format!(
                            "Command executed (Exit: {})\nSTDOUT:\n{}\nSTDERR:\n{}",
                            exit_code, stdout, stderr
                        ))
                    }
                    Err(e) => Ok(format!("Failed to execute command '{}': {}", command, e)),
                }
            }
            OrchestrationAction::StartJob { name, command } => {
                #[cfg(target_os = "windows")]
                let mut cmd = tokio::process::Command::new("powershell");
                #[cfg(target_os = "windows")]
                cmd.arg("-Command").arg(command);

                #[cfg(not(target_os = "windows"))]
                let mut cmd = tokio::process::Command::new("sh");
                #[cfg(not(target_os = "windows"))]
                cmd.arg("-c").arg(command);

                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                match cmd.spawn() {
                    Ok(child) => {
                        let id = child.id().unwrap_or(0);
                        let mut jobs = self.jobs.lock().await;
                        jobs.insert(name.clone(), child);
                        Ok(format!("Job '{}' started successfully (PID: {}).", name, id))
                    }
                    Err(e) => Ok(format!("Failed to start job '{}': {}", name, e)),
                }
            }
            OrchestrationAction::StopJob { name } => {
                let mut jobs = self.jobs.lock().await;
                if let Some(mut child) = jobs.remove(name) {
                    match child.kill().await {
                        Ok(_) => Ok(format!("Job '{}' stopped.", name)),
                        Err(e) => Ok(format!("Failed to stop job '{}': {}", name, e)),
                    }
                } else {
                    Ok(format!("Job '{}' not found.", name))
                }
            }
            OrchestrationAction::ListJobs => {
                let jobs = self.jobs.lock().await;
                if jobs.is_empty() {
                    Ok("No background jobs running.".to_string())
                } else {
                    let mut output = String::from("Running Jobs:\n");
                    for (name, child) in jobs.iter() {
                        output.push_str(&format!("- {} (PID: {:?})\n", name, child.id()));
                    }
                    Ok(output)
                }
            }
            OrchestrationAction::DelegateTask { agent, goal } => {
                let sub_agent_id = format!("{}-{}", self.agent_id, agent);
                let sub_runtime = AgentRuntime::new(
                    sub_agent_id.clone(),
                    self.engine.clone(),
                    self.registry.clone(),
                    self.project.clone(),
                    self.task.clone(),
                    self.watch.clone(),
                    self.agent.clone(),
                );

                let _ = self.agent.connect(&sub_agent_id, Some(agent)).await;
                match Box::pin(async move { sub_runtime.run_loop(goal).await }).await {
                    Ok(_) => Ok(format!("Delegated task to '{}' completed successfully.", agent)),
                    Err(e) => Ok(format!("Delegated task to '{}' failed: {}", agent, e)),
                }
            }
            OrchestrationAction::CallAgent { tool, args } => {
                if let Ok(result) = self.registry.call(tool, &EmptyContext, args.clone()).await {
                    return Ok(format!("Tool '{}' executed with result:\n{}", tool, result));
                }

                let argv = args
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let allowed_tools = ["gh", "aws", "kubectl", "cargo", "git", "docker"];
                if !allowed_tools.contains(&tool.as_str()) {
                    return Ok(format!("Security Error: Tool '{}' is not allowed.", tool));
                }

                #[cfg(target_os = "windows")]
                let mut cmd = tokio::process::Command::new("powershell");
                #[cfg(target_os = "windows")]
                cmd.arg("-Command").arg(format!("{} {}", tool, argv.join(" ")));

                #[cfg(not(target_os = "windows"))]
                let mut cmd = tokio::process::Command::new(tool);
                #[cfg(not(target_os = "windows"))]
                cmd.args(&argv);

                match cmd.output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let exit_code = output.status.code().unwrap_or(-1);
                        Ok(format!(
                            "Tool '{}' executed (Exit: {})\nSTDOUT:\n{}\nSTDERR:\n{}",
                            tool, exit_code, stdout, stderr
                        ))
                    }
                    Err(e) => Ok(format!("Failed to execute tool '{}': {}", tool, e)),
                }
            }
            OrchestrationAction::AwaitJob { job_id } => {
                let mut jobs = self.jobs.lock().await;
                if let Some(mut child) = jobs.remove(job_id.as_str()) {
                    drop(jobs);
                    match child.wait().await {
                        Ok(status) => Ok(format!("Job '{}' finished with status: {}", job_id, status)),
                        Err(e) => Ok(format!("Error waiting for job '{}': {}", job_id, e)),
                    }
                } else {
                    Ok(format!("Job '{}' not found or already finished.", job_id))
                }
            }
        }
    }
}
