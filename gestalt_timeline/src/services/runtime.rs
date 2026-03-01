use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use crate::models::{AgentRuntimeState, EventType, RuntimePhase, TimelineEvent};
use crate::services::{
    spawn_reviewer_agent, AgentService, ContextCompactor, LockStatus, MemoryService, OverlayFs,
    ProjectService, ReviewerMessage, TaskService, TimelineService, VirtualFs, WatchService,
};
use synapse_agentic::prelude::{DecisionContext, DecisionEngine, EmptyContext, Hive, ToolRegistry};

/// Orchestration action executed by AgentRuntime.
#[derive(Debug, Clone)]
pub enum OrchestrationAction {
    CreateProject {
        name: String,
        description: Option<String>,
    },
    CreateTask {
        project: String,
        description: String,
    },
    RunTask {
        task_id: String,
    },
    ListProjects,
    ListTasks {
        project: Option<String>,
    },
    GetStatus {
        project: String,
    },
    Chat {
        response: String,
    },
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
        content: String,
    },
    FlushVfs,
    ExecuteShell {
        command: String,
    },
    StartJob {
        name: String,
        command: String,
    },
    StopJob {
        name: String,
    },
    ListJobs,
    DelegateTask {
        agent: String,
        goal: String,
    },
    CallAgent {
        tool: String,
        args: Value,
    },
    AwaitJob {
        job_id: String,
    },
    ReviewAndMerge {
        goal: String,
    },
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
    timeline: TimelineService,
    memory: MemoryService,
    watch: WatchService,
    agent: AgentService,
    hard_step_cap: Option<usize>,
    jobs: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
    vfs: Arc<dyn VirtualFs>,
    compactor: ContextCompactor,
    hive: Arc<Mutex<Hive>>,
}

struct PersistStateInput<'a> {
    goal: &'a str,
    step: usize,
    phase: RuntimePhase,
    last_action: Option<&'a OrchestrationAction>,
    last_observation: Option<&'a str>,
    history: &'a [String],
    started_at: crate::models::FlexibleTimestamp,
    finished_at: Option<crate::models::FlexibleTimestamp>,
    error: Option<String>,
}

impl AgentRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent_id: String,
        engine: Arc<DecisionEngine>,
        registry: Arc<ToolRegistry>,
        project: ProjectService,
        task: TaskService,
        timeline: TimelineService,
        watch: WatchService,
        agent: AgentService,
        memory: MemoryService,
    ) -> Self {
        // Use the first available provider from the engine for context compaction
        let compactor_provider = engine.providers().first().cloned().unwrap_or_else(|| {
            // Fallback (though engine should have at least the StochasticRotator now)
            Arc::new(synapse_agentic::prelude::GeminiProvider::new(
                "".into(),
                "gpt-4o".into(),
            ))
        });

        Self {
            agent_id,
            engine,
            registry,
            project,
            task,
            timeline,
            memory,
            watch,
            agent,
            hard_step_cap: std::env::var("GESTALT_HARD_STEP_CAP")
                .ok()
                .and_then(|v| v.parse::<usize>().ok()),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            vfs: Arc::new(OverlayFs::new()),
            compactor: ContextCompactor::new(compactor_provider, "gpt-4o"),
            hive: Arc::new(Mutex::new(Hive::new())),
        }
    }

    /// Run the autonomous loop for a specific goal.
    pub async fn run_loop(&self, goal: &str) -> Result<()> {
        info!("Starting Autonomous Loop for Agent: {}", self.agent_id);
        info!("Goal: {}", goal);

        use crate::services::AgentStatus;
        let _ = self
            .agent
            .set_status(&self.agent_id, AgentStatus::Busy)
            .await;

        let mut conversation_history = Vec::new();
        conversation_history.push(format!(
            "GOAL: {}\n\nPlease start working on this goal.",
            goal
        ));
        let started_at = crate::models::FlexibleTimestamp::now();
        self.persist_state(PersistStateInput {
            goal,
            step: 0,
            phase: RuntimePhase::Running,
            last_action: None,
            last_observation: None,
            history: &conversation_history,
            started_at: started_at.clone(),
            finished_at: None,
            error: None,
        })
        .await?;

        let loop_result: Result<()> = async {
            let mut step = 0usize;

            loop {
                if let Some(limit) = self.hard_step_cap {
                    if step >= limit {
                        warn!("Hard safety cap reached at {} steps.", limit);
                        self.persist_state(PersistStateInput {
                            goal,
                            step,
                            phase: RuntimePhase::Completed,
                            last_action: None,
                            last_observation: Some("Elastic loop stopped by hard safety cap."),
                            history: &conversation_history,
                            started_at: started_at.clone(),
                            finished_at: Some(crate::models::FlexibleTimestamp::now()),
                            error: None,
                        })
                        .await?;
                        break;
                    }
                }

                let outcome = self.compactor.compact(&mut conversation_history).await;
                if outcome.compacted {
                    info!(
                        "Context compacted: {} -> {} tokens",
                        outcome.tokens_before, outcome.tokens_after
                    );
                }

                step += 1;
                info!("Elastic step {}", step);
                let actions = self.next_actions(goal, &conversation_history).await?;

                if actions.is_empty() {
                    info!("Agent decided to stop (no actions).");
                    self.persist_state(PersistStateInput {
                        goal,
                        step,
                        phase: RuntimePhase::Completed,
                        last_action: None,
                        last_observation: Some("Agent returned no actions; loop stopped."),
                        history: &conversation_history,
                        started_at: started_at.clone(),
                        finished_at: Some(crate::models::FlexibleTimestamp::now()),
                        error: None,
                    })
                    .await?;
                    break;
                }

                for action in actions {
                    conversation_history.push(format!("Action: {:?}", action));
                    let observation = match self.execute_action(&action).await {
                        Ok(obs) => obs,
                        Err(e) => format!("Error: {}", e),
                    };
                    conversation_history.push(format!("Observation: {}", observation));
                    self.persist_state(PersistStateInput {
                        goal,
                        step,
                        phase: RuntimePhase::Running,
                        last_action: Some(&action),
                        last_observation: Some(&observation),
                        history: &conversation_history,
                        started_at: started_at.clone(),
                        finished_at: None,
                        error: None,
                    })
                    .await?;
                }
            }
            Ok(())
        }
        .await;

        let _ = self
            .agent
            .set_status(&self.agent_id, AgentStatus::Idle)
            .await;
        if let Err(e) = loop_result {
            let _ = self
                .persist_state(PersistStateInput {
                    goal,
                    step: 0,
                    phase: RuntimePhase::Failed,
                    last_action: None,
                    last_observation: Some("Loop failed."),
                    history: &conversation_history,
                    started_at,
                    finished_at: Some(crate::models::FlexibleTimestamp::now()),
                    error: Some(e.to_string()),
                })
                .await;
            self.vfs.release_locks(&self.agent_id).await;
            return Err(e);
        }

        self.vfs.release_locks(&self.agent_id).await;

        Ok(())
    }

    async fn persist_state(&self, input: PersistStateInput<'_>) -> Result<()> {
        let mut state =
            AgentRuntimeState::new(&self.agent_id, input.goal, self.hard_step_cap.unwrap_or(0));
        state.phase = input.phase;
        state.current_step = input.step;
        state.last_action = input.last_action.map(|a| format!("{:?}", a));
        state.last_observation = input.last_observation.map(ToOwned::to_owned);
        state.history_tail = input
            .history
            .iter()
            .rev()
            .take(20)
            .cloned()
            .collect::<Vec<_>>();
        state.history_tail.reverse();
        state.started_at = input.started_at;
        state.updated_at = crate::models::FlexibleTimestamp::now();
        state.finished_at = input.finished_at;
        state.error = input.error;

        let db = self.watch.db();
        let _saved: AgentRuntimeState = db
            .upsert("agent_runtime_states", &self.agent_id, &state)
            .await?;
        Ok(())
    }

    async fn next_actions(
        &self,
        goal: &str,
        history: &[String],
    ) -> Result<Vec<OrchestrationAction>> {
        let history_summary = history
            .iter()
            .rev()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        let context = DecisionContext::new("orchestration")
            .with_summary(format!("goal: {}\n\nhistory:\n{}", goal, history_summary));

        // Use native engine resilience (configured with StochasticRotator in main)
        let decision = self.engine.decide(&context).await?;
        Ok(Self::map_decision_to_actions(
            &decision.action,
            &decision.reasoning,
        ))
    }


    fn map_decision_to_actions(action: &str, reasoning: &str) -> Vec<OrchestrationAction> {
        match action.trim().to_lowercase().as_str() {
            "stop" | "done" | "defer" => vec![],
            "list_projects" => vec![OrchestrationAction::ListProjects],
            "list_jobs" => vec![OrchestrationAction::ListJobs],
            "flush_vfs" => vec![OrchestrationAction::FlushVfs],
            "review_merge" => vec![OrchestrationAction::ReviewAndMerge {
                goal: reasoning.to_string(),
            }],
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
            OrchestrationAction::CreateProject {
                name,
                description: _description,
            } => {
                self.project.create_project(name, &self.agent_id).await?;
                Ok(format!("Created Project '{}'", name))
            }
            OrchestrationAction::CreateTask {
                project,
                description,
            } => {
                self.task
                    .create_task(project, description, &self.agent_id)
                    .await?;
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
                Ok(format!(
                    "Project '{}' is {}% complete",
                    project, status.progress_percent
                ))
            }
            OrchestrationAction::Chat { response } => {
                info!("Agent says: {}", response);
                let _ = self.timeline.record_event(
                    TimelineEvent::new(&self.agent_id, EventType::ChatMessage)
                        .with_payload(serde_json::json!({
                            "text": response,
                            "sender": self.agent_id
                        }))
                ).await;
                Ok(format!("Agent said: {}", response))
            }
            OrchestrationAction::ReadFile { path } => {
                match self.vfs.read_to_string(Path::new(path)).await {
                    Ok(content) => Ok(format!("File '{}' content:\n{}", path, content)),
                    Err(e) => Ok(format!("Error reading file '{}': {}", path, e)),
                }
            }
            OrchestrationAction::WriteFile { path, content } => {
                if let Some(parent) = Path::new(path).parent() {
                    self.vfs.create_dir_all(parent).await?;
                }
                let lock_status = self
                    .vfs
                    .acquire_lock(Path::new(path), &self.agent_id)
                    .await?;
                match lock_status {
                    LockStatus::HeldByOther { owner } => {
                        let _ = self
                            .timeline
                            .record_event(
                                TimelineEvent::new(&self.agent_id, EventType::VfsLockConflict)
                                    .with_payload(serde_json::json!({
                                        "path": path,
                                        "owner": owner,
                                    })),
                            )
                            .await;
                        return Ok(format!(
                            "Lock conflict for '{}': currently held by '{}'.",
                            path, owner
                        ));
                    }
                    LockStatus::Acquired => {
                        let _ = self
                            .timeline
                            .record_event(
                                TimelineEvent::new(&self.agent_id, EventType::VfsLockAcquired)
                                    .with_payload(serde_json::json!({
                                        "path": path,
                                    })),
                            )
                            .await;
                    }
                    _ => {}
                }

                match self
                    .vfs
                    .write_string(Path::new(path), content.clone(), &self.agent_id)
                    .await
                {
                    Ok(_) => {
                        let version = self.vfs.version().await;
                        let _ = self
                            .timeline
                            .record_event(
                                TimelineEvent::new(&self.agent_id, EventType::VfsPatchApplied)
                                    .with_payload(serde_json::json!({
                                        "path": path,
                                        "version": version,
                                    })),
                            )
                            .await;
                        Ok(format!("Successfully wrote to file '{}'", path))
                    }
                    Err(e) => Ok(format!("Error writing file '{}': {}", path, e)),
                }
            }
            OrchestrationAction::FlushVfs => {
                let _ = self
                    .timeline
                    .emit(&self.agent_id, EventType::VfsFlushStarted)
                    .await;
                match self.vfs.flush().await {
                    Ok(report) => {
                        let version = self.vfs.version().await;
                        let _ = self
                            .timeline
                            .record_event(
                                TimelineEvent::new(&self.agent_id, EventType::VfsFlushCompleted)
                                    .with_payload(serde_json::json!({
                                        "version": version,
                                        "files": report.written_files.len(),
                                        "dirs": report.created_dirs.len(),
                                        "errors": report.errors.len(),
                                    })),
                            )
                            .await;
                        Ok(format!(
                            "VFS flush complete. dirs={}, files={}, errors={}",
                            report.created_dirs.len(),
                            report.written_files.len(),
                            report.errors.len()
                        ))
                    }
                    Err(e) => Ok(format!("VFS flush failed: {}", e)),
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
                        Ok(format!(
                            "Job '{}' started successfully (PID: {}).",
                            name, id
                        ))
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
                    self.timeline.clone(),
                    self.watch.clone(),
                    self.agent.clone(),
                    self.memory.clone(),
                );

                let _ = self.agent.connect(&sub_agent_id, Some(agent)).await;
                match Box::pin(async move { sub_runtime.run_loop(goal).await }).await {
                    Ok(_) => Ok(format!(
                        "Delegated task to '{}' completed successfully.",
                        agent
                    )),
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
                cmd.arg("-Command")
                    .arg(format!("{} {}", tool, argv.join(" ")));

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
                        Ok(status) => {
                            Ok(format!("Job '{}' finished with status: {}", job_id, status))
                        }
                        Err(e) => Ok(format!("Error waiting for job '{}': {}", job_id, e)),
                    }
                } else {
                    Ok(format!("Job '{}' not found or already finished.", job_id))
                }
            }
            OrchestrationAction::ReviewAndMerge { goal } => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let handle = {
                    let mut hive = self.hive.lock().await;
                    spawn_reviewer_agent(&mut hive)
                };
                if let Err(e) = handle
                    .send(ReviewerMessage::ReviewAndMerge {
                        goal: goal.clone(),
                        reply: reply_tx,
                    })
                    .await
                {
                    return Ok(format!("Reviewer agent dispatch failed: {}", e));
                }

                match reply_rx.await {
                    Ok(result) => Ok(format!(
                        "Reviewer decision: approved={} summary={}",
                        result.approved, result.summary
                    )),
                    Err(e) => Ok(format!("Reviewer did not respond: {}", e)),
                }
            }
        }
    }
}
