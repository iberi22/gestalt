use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tokio::task::JoinSet;
use uuid::Uuid;

use gestalt_core::application::agent::xavier::XavierClient;
use gestalt_state::memstate::MemState;
use gestalt_state::statedb::StateDb;
use gestalt_state::AgentState;
use gestalt_ws::WsEvent;
use gestalt_ws::WsServer;

use crate::agent::AgentRunner;
use crate::run::{AgentResult, RouterError, RunReport, RunSpec};
use crate::overlap::OverlapDetector;
use crate::timeline::{Event, EventLog};
use crate::worktree::WorktreeManager;
use gestalt_core::ports::outbound::vfs::VirtualFS;

pub struct Router {
    pub vfs: Option<Arc<dyn VirtualFS>>,
    pub runner: Arc<dyn AgentRunner>,
    pub state_db: Arc<StateDb>,
    pub mem_state: MemState,
    pub log: Option<Arc<dyn EventLog>>,
    pub xavier: Option<XavierClient>,
    /// Optional WebSocket server for broadcasting timeline events.
    pub ws_server: Option<WsServer>,
    /// Internal WorktreeManager for git worktree operations (legacy).
    worktrees: Arc<WorktreeManager>,
}

impl Router {
    /// Create a new Router with StateDb and MemState instead of EventLog.
    ///
    /// The EventLog is now optional — timeline events can be logged
    /// via the existing JsonlEventLog for backward compatibility, but
    /// all persistent state lives in StateDb.
    ///
    /// `ws_server` can be provided to broadcast timeline events to
    /// WebSocket clients in real time.
    pub fn new(
        vfs: Option<Arc<dyn VirtualFS>>,
        runner: Arc<dyn AgentRunner>,
        state_db: Arc<StateDb>,
        mem_state: MemState,
        log: Option<Arc<dyn EventLog>>,
        ws_server: Option<WsServer>,
    ) -> Self {
        Self {
            vfs,
            runner,
            state_db,
            mem_state,
            log,
            xavier: None,
            ws_server,
            worktrees: Arc::new(WorktreeManager::new(
                std::path::PathBuf::from("/tmp/gestalt"),
            )),
        }
    }

    /// Attach an optional Xavier client for memory/context integration.
    pub fn with_xavier(mut self, xavier: XavierClient) -> Self {
        self.xavier = Some(xavier);
        self
    }

    /// Attach a WebSocket server for timeline event broadcasting.
    pub fn with_ws_server(mut self, ws_server: WsServer) -> Self {
        self.ws_server = Some(ws_server);
        self
    }

    /// Resolves the base_ref to a git commit SHA.
    pub fn resolve_base_sha(&self, base_ref: &str) -> Result<String, RouterError> {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", &format!("{}^{{commit}}", base_ref)])
            .output()
            .map_err(|e| RouterError::GitError(format!("Failed to spawn git rev-parse: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RouterError::InvalidSpec(format!(
                "Failed to resolve base_ref '{}' to commit SHA: {}",
                base_ref,
                stderr.trim()
            )));
        }

        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if sha.is_empty() {
            return Err(RouterError::InvalidSpec(format!(
                "Empty SHA resolved for base_ref '{}'",
                base_ref
            )));
        }

        Ok(sha)
    }

    /// Log a timeline event if an EventLog is attached.
    fn log_event(&self, event: Event) {
        if let Some(ref log) = self.log {
            let _ = log.log(event);
        }
    }

    /// Broadcast a WsEvent through the optional WebSocket server.
    fn broadcast_ws_event(&self, event: WsEvent) {
        if let Some(ref ws) = self.ws_server {
            let ws = ws.clone();
            let event = event.clone();
            tokio::spawn(async move {
                ws.broadcast(&event).await;
            });
        }
    }

    /// Main Router pipeline execution using StateDb + MemState.
    pub async fn execute(&self, mut spec: RunSpec) -> Result<RunReport, RouterError> {
        // 1. Validate spec
        if spec.agents.is_empty() {
            return Err(RouterError::InvalidSpec(
                "No agents specified in RunSpec".to_string(),
            ));
        }

        let base_sha = self.resolve_base_sha(&spec.base_ref)?;

        // Generate Run ID
        let run_id = Uuid::new_v4();
        let run_id_str = run_id.to_string();
        let _timer_start = std::time::Instant::now();

        // 2. Initialize StateDb with the run
        let spec_json = serde_json::to_string(&spec)
            .map_err(|e| RouterError::InvalidSpec(format!("Failed to serialize spec: {}", e)))?;
        self.state_db
            .create_run(&run_id_str, &spec_json)
            .map_err(|e| {
                RouterError::InvalidSpec(format!("Failed to create run in StateDb: {}", e))
            })?;

        // 3. PRE: Fetch relevant context from Xavier memory and inject into agents
        let xavier_context = if let Some(ref xavier) = self.xavier {
            let ctx = xavier.search_context(&spec.task, 5).await;
            if !ctx.is_empty() {
                tracing::info!(
                    "Xavier context fetched for task: {} results (run {})",
                    ctx.len(),
                    run_id
                );
                Some(ctx)
            } else {
                tracing::info!(
                    "No Xavier context found for task (run {})",
                    run_id
                );
                None
            }
        } else {
            None
        };

        // Inject XAVIER_CONTEXT into each agent's environment
        if let Some(ref ctx) = xavier_context {
            let ctx_json = serde_json::to_string(ctx).unwrap_or_default();
            for agent in &mut spec.agents {
                let env = agent.env.get_or_insert_with(HashMap::new);
                env.insert("XAVIER_CONTEXT".into(), ctx_json.clone());
            }
        }

        // 4. Fire RunStarted event
        let agent_ids: Vec<String> = spec.agents.iter().map(|a| a.id.clone()).collect();
        self.log_event(Event::RunStarted {
            run_id,
            sha_base: base_sha.clone(),
            agents: agent_ids.clone(),
            task: spec.task.clone(),
        });
        self.broadcast_ws_event(WsEvent::RunStarted {
            run_id: run_id.to_string(),
            task: spec.task.clone(),
            agents: agent_ids,
        });

        // 5. Create Worktrees (serial)
        let mut created_wts = Vec::new();
        let mut wt_paths = HashMap::new();

        for agent in &spec.agents {
            match self.worktrees.create_worktree(run_id, &agent.id, &base_sha) {
                Ok(path) => {
                    created_wts.push(path.clone());
                    wt_paths.insert(agent.id.clone(), path);
                }
                Err(e) => {
                    // Cleanup already created worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt);
                    }
                    return Err(RouterError::GitError(format!(
                        "Failed to create worktree for agent {}: {}",
                        agent.id, e
                    )));
                }
            }
        }

        // 6. Spawn agents in parallel using JoinSet
        //    No Semaphore — file-level concurrency uses MemState::try_lock
        let mut join_set: JoinSet<Result<AgentResult, RouterError>> = JoinSet::new();

        for agent in spec.agents {
            let agent_id = agent.id.clone();
            let agent_spec = agent.clone();
            let task_desc = spec.task.clone();
            let run_id_clone = run_id;
            let run_id_str_clone = run_id_str.clone();
            let wt_path = wt_paths.get(&agent_id).cloned().unwrap();
            let timeout = std::time::Duration::from_secs(spec.timeout);

            let runner_clone = self.runner.clone();
            let state_db = self.state_db.clone();
            let mem_state = self.mem_state.clone();
            let log_opt = self.log.clone();
            let ws_server = self.ws_server.clone();

            join_set.spawn(async move {
                // Set agent state to Running in MemState
                mem_state.set_agent_state(
                    &run_id_str_clone,
                    &agent_id,
                    &AgentState::Running.to_string(),
                );

                // Log state change
                if let Some(ref log) = log_opt {
                    let _ = log.log(Event::AgentStateChanged {
                        run_id: run_id_clone,
                        agent_id: agent_id.clone(),
                        from: AgentState::Pending,
                        to: AgentState::Running,
                    });
                }

                // Broadcast state change via WebSocket
                if let Some(ref ws) = ws_server {
                    ws.broadcast(&WsEvent::StateChanged {
                        run_id: run_id_str_clone.clone(),
                        agent_id: agent_id.clone(),
                        state: "running".to_string(),
                    })
                    .await;
                }

                // Check for lock conflicts before running the agent
                let conflicts = OverlapDetector::check_all_locks_for_agent(
                    &mem_state,
                    &agent_id,
                );
                for (path, holder_id) in &conflicts {
                    let conflict_payload = serde_json::json!({
                        "path": path,
                        "agent_a": holder_id,
                        "agent_b": agent_id,
                        "message": format!(
                            "Agent {agent_id} may conflict with agent {holder_id} on {path}"
                        ),
                    })
                    .to_string();
                    mem_state.push_event(
                        &run_id_str_clone,
                        Some(&agent_id),
                        "conflict_detected",
                        &conflict_payload,
                    );
                    // Broadcast via WebSocket if available
                    if let Some(ref ws) = ws_server {
                        ws.broadcast(&WsEvent::ConflictDetected {
                            run_id: run_id_str_clone.clone(),
                            agent_a: holder_id.clone(),
                            agent_b: agent_id.clone(),
                            path: path.clone(),
                            message: format!(
                                "Agent {agent_id} may conflict with agent {holder_id} on {path}"
                            ),
                        })
                        .await;
                    }
                }

                // Run Agent
                let mut run_result = runner_clone
                    .run(&agent_spec, &wt_path, &task_desc, timeout)
                    .await?;

                // Run Checkpoint
                let checkpoint_res = crate::checkpoint::run_checkpoint(&wt_path, &agent_id);

                let final_state = match run_result.state {
                    AgentState::Success => match checkpoint_res {
                        Ok(true) => AgentState::Success,
                        Ok(false) => AgentState::NoChanges,
                        Err(e) => {
                            run_result.error = Some(format!("Checkpoint failed: {}", e));
                            AgentState::Crashed
                        }
                    },
                    AgentState::Crashed => {
                        let _ = checkpoint_res;
                        AgentState::Crashed
                    }
                    AgentState::Timeout => {
                        let _ = checkpoint_res;
                        AgentState::Timeout
                    }
                    other => other,
                };

                run_result.state = final_state.clone();

                // Update final state in MemState
                mem_state.set_agent_state(&run_id_str_clone, &agent_id, &final_state.to_string());

                // Persist agent result to StateDb using its upsert API
                let changed_files_json = serde_json::to_string(&run_result.changed_files)
                    .unwrap_or_else(|_| "[]".to_string());
                let _ = state_db.upsert_agent(
                    &run_id_str_clone,
                    &agent_id,
                    &final_state.to_string(),
                    run_result.output.as_deref(),
                    run_result.error.as_deref(),
                    run_result.duration_ms as i64,
                    &changed_files_json,
                );

                // Log final state change
                if let Some(ref log) = log_opt {
                    let _ = log.log(Event::AgentStateChanged {
                        run_id: run_id_clone,
                        agent_id: agent_id.clone(),
                        from: AgentState::Running,
                        to: final_state.clone(),
                    });
                }

                // Broadcast final state change via WebSocket
                if let Some(ref ws) = ws_server {
                    ws.broadcast(&WsEvent::StateChanged {
                        run_id: run_id_str_clone.clone(),
                        agent_id: agent_id.clone(),
                        state: format!("{:?}", final_state).to_lowercase(),
                    })
                    .await;
                }

                Ok::<AgentResult, RouterError>(run_result)
            });
        }

        // Wait for all agents to finish
        let mut agent_results = Vec::new();
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(agent_result)) => {
                    agent_results.push(agent_result);
                }
                Ok(Err(e)) => {
                    // Cleanup worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt);
                    }
                    return Err(e);
                }
                Err(e) => {
                    // Cleanup worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt);
                    }
                    return Err(RouterError::AgentError(format!(
                        "Agent task panicked or cancelled: {}",
                        e
                    )));
                }
            }
        }

        // 7. Overlap detection
        let active_branches: Vec<(String, String)> = agent_results
            .iter()
            .filter(|r| r.state == AgentState::Success || r.state == AgentState::Crashed)
            .map(|r| {
                (
                    r.agent_id.clone(),
                    format!("gestalt/{}/{}", run_id, r.agent_id),
                )
            })
            .collect();

        let overlaps =
            crate::overlap::find_overlaps(std::path::Path::new("."), &base_sha, &active_branches)?;
        for overlap in &overlaps {
            self.log_event(Event::OverlapDetected {
                run_id,
                agent_a: overlap.agent_a.clone(),
                agent_b: overlap.agent_b.clone(),
                files: overlap
                    .files
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            });
        }

        // 8. Sequential branch integration
        let branches_to_merge: Vec<(String, String)> = agent_results
            .iter()
            .filter(|r| r.state == AgentState::Success)
            .map(|r| {
                (
                    r.agent_id.clone(),
                    format!("gestalt/{}/{}", run_id, r.agent_id),
                )
            })
            .collect();

        let mut merged_branches = Vec::new();
        let mut conflicts = Vec::new();

        if !branches_to_merge.is_empty() {
            match self
                .worktrees
                .create_worktree(run_id, "_integrate", &base_sha)
            {
                Ok(integrate_wt_path) => {
                    match crate::integrate::integrate_branches(
                        &integrate_wt_path,
                        &base_sha,
                        spec.integration_branch.as_deref().unwrap_or("main"),
                        &branches_to_merge,
                    ) {
                        Ok(integration_res) => {
                            merged_branches = integration_res.merged_branches;
                            conflicts = integration_res.conflicts;
                        }
                        Err(e) => {
                            let _ = self.worktrees.cleanup_worktree(&integrate_wt_path);
                            // Cleanup other worktrees
                            for wt in &created_wts {
                                let _ = self.worktrees.cleanup_worktree(wt);
                            }
                            return Err(e);
                        }
                    }
                    let _ = self.worktrees.cleanup_worktree(&integrate_wt_path);
                }
                Err(e) => {
                    // Cleanup other worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt);
                    }
                    return Err(e);
                }
            }
        }

        // Log Merge Conflicts
        for conflict in &conflicts {
            self.log_event(Event::MergeConflict {
                run_id,
                agent: conflict.agent_id.clone(),
                path: conflict.path.clone(),
            });
        }

        // 9. Cleanup agent worktrees
        for wt in &created_wts {
            let _ = self.worktrees.cleanup_worktree(wt);
        }

        // 10. Fire RunFinished Event and complete StateDb run
        let summary = format!(
            "Completed run with {} agents. Merged: {}. Conflicts: {}.",
            agent_results.len(),
            merged_branches.len(),
            conflicts.len()
        );
        self.log_event(Event::RunFinished {
            run_id,
            summary: summary.clone(),
        });
        self.broadcast_ws_event(WsEvent::RunFinished {
            run_id: run_id.to_string(),
            summary: summary.clone(),
        });

        // Mark run as completed in StateDb
        self.state_db
            .complete_run(&run_id_str, "completed")
            .map_err(|e| {
                RouterError::InvalidSpec(format!("Failed to complete run in StateDb: {}", e))
            })?;

        // 11. POST: Archive run results as memory in Xavier
        let events_path = self
            .worktrees
            .base_dir
            .join(run_id.to_string())
            .join("events.jsonl")
            .to_string_lossy()
            .to_string();
        let duration_ms = _timer_start.elapsed().as_millis() as u64;
        if let Some(ref xavier) = self.xavier {
            let content = serde_json::to_string_pretty(&RunReport {
                run_id,
                task: spec.task.clone(),
                agents: agent_results.clone(),
                duration_ms,
                merged_branches: merged_branches.clone(),
                conflicts: conflicts.clone(),
                events_path: events_path.clone(),
                success: true,
            }.to_json())
                .unwrap_or_else(|_| "{}".to_string());
            let metadata = serde_json::json!({
                "run_id": run_id.to_string(),
                "task": spec.task,
                "agents": agent_results.len(),
                "duration_ms": duration_ms,
                "success": true,
            });

            match xavier.archive_run(&content, &run_id_str, metadata).await {
                Ok(memory_id) => {
                    tracing::info!(
                        "Xavier memory stored for run {} (memory_id={})",
                        run_id,
                        memory_id
                    );
                }
                Err(e) => {
                    tracing::warn!("Xavier archive_run failed (non-fatal): {}", e);
                }
            }
        }

        Ok(RunReport {
            run_id,
            task: spec.task,
            agents: agent_results,
            duration_ms,
            merged_branches,
            conflicts,
            events_path,
            success: true,
        })
    }
}
