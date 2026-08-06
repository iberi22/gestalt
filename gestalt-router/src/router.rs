use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tokio::task::JoinSet;
use uuid::Uuid;

use gestalt_core::application::agent::xavier::XavierClient;
use gestalt_core::ports::outbound::search::LocalSearchEngine;
use gestalt_state::memstate::MemState;
use gestalt_state::statedb::StateDb;
use gestalt_state::AgentState;
use gestalt_ws::WsEvent;
use gestalt_ws::WsServer;

use crate::agent::AgentRunner;
use crate::event_bus::BusEvent;
use crate::overlap::{LiveConflictDetector, OverlapDetector};
use crate::run::{AgentResult, ConflictInfo, RouterError, RunReport, RunSpec};
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
    /// Local BM25 search engine for offline context retrieval.
    pub search_engine: Option<Arc<dyn LocalSearchEngine>>,
    /// Optional WebSocket server for broadcasting timeline events.
    pub ws_server: Option<WsServer>,
    /// Internal WorktreeManager for git worktree operations (legacy).
    worktrees: Arc<WorktreeManager>,
    /// Enable dry run mode (simulate merges without writing).
    pub dry_run: bool,
}

impl Router {
    /// Create a new Router with StateDb and MemState instead of EventLog.
    ///
    /// The EventLog is now optional — timeline events can be logged
    /// via the existing StateDbEventLog for backward compatibility, but
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
        let router = Self {
            vfs,
            runner,
            state_db,
            mem_state,
            log,
            xavier: None,
            search_engine: None,
            ws_server,
            worktrees: Arc::new(WorktreeManager::new(std::path::PathBuf::from(
                "/tmp/gestalt",
            ))),
            dry_run: false,
        };

        // Spawn LiveConflictDetector if a WebSocket server is configured
        if router.ws_server.is_some() {
            let detector =
                LiveConflictDetector::new(router.mem_state.clone(), router.ws_server.clone());
            tokio::spawn(detector.run());
        }

        router
    }

    /// Attach an optional Xavier client for memory/context integration.
    pub fn with_xavier(mut self, xavier: XavierClient) -> Self {
        self.xavier = Some(xavier);
        self
    }

    /// Attach a local BM25 search engine for offline context retrieval.
    pub fn with_search_engine(mut self, engine: Arc<dyn LocalSearchEngine>) -> Self {
        self.search_engine = Some(engine);
        self
    }

    /// Attach a WebSocket server for timeline event broadcasting.
    pub fn with_ws_server(mut self, ws_server: WsServer) -> Self {
        self.ws_server = Some(ws_server);
        self
    }

    /// Enable dry run mode (simulate merges without writing).
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
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

    /// Fetch context for a task using local BM25 or Xavier remote.
    ///
    /// Priority:
    /// 1. Local BM25 search engine (fast, offline)
    /// 2. Xavier remote (if XavierClient is configured)
    ///
    /// Returns `None` if both are unavailable or return no results.
    async fn fetch_context(&self, task: &str, limit: usize) -> Option<Vec<String>> {
        // Try local BM25 first (offline-capable)
        if let Some(ref engine) = self.search_engine {
            match engine.search(task, limit).await {
                Ok(results) if !results.is_empty() => {
                    let ctx: Vec<String> = results
                        .into_iter()
                        .map(|r| {
                            if r.snippet.is_empty() {
                                r.content
                            } else {
                                r.snippet
                            }
                        })
                        .collect();
                    tracing::info!("Local BM25 context fetched for task: {} results", ctx.len());
                    return Some(ctx);
                },
                Ok(_) => {
                    tracing::info!("Local BM25 returned no results for task");
                },
                Err(e) => {
                    tracing::warn!("Local BM25 search failed (non-fatal): {}", e);
                },
            }
        }

        // Fall back to Xavier remote
        if let Some(ref xavier) = self.xavier {
            let ctx = xavier.search_context(task, limit).await;
            if !ctx.is_empty() {
                tracing::info!("Xavier context fetched for task: {} results", ctx.len());
                return Some(ctx);
            }
            tracing::info!("No Xavier context found for task");
        }

        None
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
            let redacted_event = crate::xavier_sink::redact_ws_event(event);
            tokio::spawn(async move {
                ws.broadcast(&redacted_event).await;
            });
        }
    }

    /// Emit a BusEvent to the universal event bus: persisted durably in the
    /// shared StateDb timeline (same store the `bus serve` HTTP endpoint
    /// serves) and streamed to Xavier as `kind=execution` when available.
    ///
    /// This is the missing link of the xavier-thinking-bus design: every
    /// orchestrated run now registers on the bus in real time, giving full
    /// traceability (agent, state, llm/provider via metadata, requested_by).
    fn emit_bus_event(&self, ev: BusEvent) {
        // Persist to the shared timeline (dedup + sink happen here).
        let db = self.state_db.clone();
        let xavier = self.xavier.clone();
        tokio::spawn(async move {
            let sink = xavier.map(crate::xavier_sink::XavierEventSink::new);
            let _ = crate::event_bus::handle_event(&db, &ev, sink.as_ref()).await;
        });
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

        // 3. PRE: Fetch relevant context from local BM25 (offline) or Xavier (remote)
        let search_context = self.fetch_context(&spec.task, 5).await;

        // Inject SEARCH_CONTEXT into each agent's environment
        if let Some(ref ctx) = search_context {
            let ctx_json = serde_json::to_string(ctx).unwrap_or_default();
            for agent in &mut spec.agents {
                let env = agent.env.get_or_insert_with(HashMap::new);
                env.insert("SEARCH_CONTEXT".into(), ctx_json.clone());
                // Backward compatibility: also inject XAVIER_CONTEXT
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

        // 4b. Emit RunStarted on the universal event bus (traceability).
        self.emit_bus_event(
            BusEvent::new("gestalt", "run_started", format!("{} agents: {}", agent_ids.len(), spec.task))
                .with_run_id(run_id.to_string())
                .with_project(std::env::var("GESTALT_PROJECT").unwrap_or_else(|_| "default".into()))
                .with_state("Running")
                .with_metadata(serde_json::json!({
                    "agents": agent_ids,
                    "base_sha": base_sha,
                    "requested_by": std::env::var("GESTALT_REQUESTED_BY").unwrap_or_else(|_| "cli".into()),
                })),
        );
        self.broadcast_ws_event(WsEvent::RunStarted {
            run_id: run_id.to_string(),
            task: spec.task.clone(),
            agents: agent_ids,
        });

        // 5. Create Worktrees (serial)
        let mut created_wts = Vec::new();
        let mut wt_paths = HashMap::new();

        for agent in &spec.agents {
            match self
                .worktrees
                .create_worktree(run_id, &agent.id, &base_sha)
                .await
            {
                Ok(path) => {
                    created_wts.push(path.clone());
                    wt_paths.insert(agent.id.clone(), path);
                },
                Err(e) => {
                    // Cleanup already created worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt).await;
                    }
                    return Err(RouterError::GitError(format!(
                        "Failed to create worktree for agent {}: {}",
                        agent.id, e
                    )));
                },
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
                let conflicts = OverlapDetector::check_all_locks_for_agent(&mem_state, &agent_id);
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
                        },
                    },
                    AgentState::Crashed => {
                        let _ = checkpoint_res;
                        AgentState::Crashed
                    },
                    AgentState::Timeout => {
                        let _ = checkpoint_res;
                        AgentState::Timeout
                    },
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
                },
                Ok(Err(e)) => {
                    // Cleanup worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt).await;
                    }
                    return Err(e);
                },
                Err(e) => {
                    // Cleanup worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt).await;
                    }
                    return Err(RouterError::AgentError(format!(
                        "Agent task panicked or cancelled: {}",
                        e
                    )));
                },
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

        // 8. Sequential branch integration via SerialMergeQueue
        let mut merged_branches = Vec::new();
        let mut conflicts = Vec::new();

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

        if !branches_to_merge.is_empty() {
            match self
                .worktrees
                .create_worktree(run_id, "_integrate", &base_sha)
                .await
            {
                Ok(integrate_wt_path) => {
                    let mut queue = SerialMergeQueue::new(
                        integrate_wt_path.clone(),
                        base_sha.clone(),
                        self.dry_run,
                    );

                    for (agent_id, branch) in &branches_to_merge {
                        if let Err(e) = queue.enqueue_and_merge(agent_id, branch) {
                            let _ = self.worktrees.cleanup_worktree(&integrate_wt_path).await;
                            // Cleanup other worktrees
                            for wt in &created_wts {
                                let _ = self.worktrees.cleanup_worktree(wt).await;
                            }
                            return Err(e);
                        }
                    }

                    merged_branches = queue.merged_branches.clone();
                    conflicts = queue.conflicts.clone();

                    if let Err(e) = queue.finish(spec.integration_branch.as_deref()) {
                        let _ = self.worktrees.cleanup_worktree(&integrate_wt_path).await;
                        // Cleanup other worktrees
                        for wt in &created_wts {
                            let _ = self.worktrees.cleanup_worktree(wt).await;
                        }
                        return Err(e);
                    }

                    let _ = self.worktrees.cleanup_worktree(&integrate_wt_path).await;
                },
                Err(e) => {
                    // Cleanup other worktrees
                    for wt in &created_wts {
                        let _ = self.worktrees.cleanup_worktree(wt).await;
                    }
                    return Err(e);
                },
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
            let _ = self.worktrees.cleanup_worktree(wt).await;
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

        // 10b. Emit RunFinished on the universal event bus (traceability).
        let run_id_str_for_bus = run_id.to_string();
        self.emit_bus_event(
            BusEvent::new("gestalt", "run_finished", summary.clone())
                .with_run_id(run_id_str_for_bus.clone())
                .with_project(std::env::var("GESTALT_PROJECT").unwrap_or_else(|_| "default".into()))
                .with_state("Success")
                .with_metadata(serde_json::json!({
                    "agents": agent_results.len(),
                    "merged_branches": merged_branches.len(),
                    "conflicts": conflicts.len(),
                    "requested_by": std::env::var("GESTALT_REQUESTED_BY").unwrap_or_else(|_| "cli".into()),
                })),
        );
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

        // 11. POST: Archive run results — local BM25 (always) + Xavier (if available)
        let report = RunReport {
            run_id,
            task: spec.task.clone(),
            agents: agent_results.clone(),
            duration_ms,
            merged_branches: merged_branches.clone(),
            conflicts: conflicts.clone(),
            events_path: events_path.clone(),
            success: true,
        };
        let content =
            serde_json::to_string_pretty(&report.to_json()).unwrap_or_else(|_| "{}".to_string());

        // 11a. Archive locally in BM25 (always, for offline search)
        if let Some(ref engine) = self.search_engine {
            let run_path = format!("gestalt/run/{}", run_id);
            if let Err(e) = engine
                .index_document(&run_id_str, &run_path, &content, "run_result")
                .await
            {
                tracing::warn!("Local BM25 archival failed (non-fatal): {}", e);
            } else {
                tracing::info!("Run {} archived in local BM25 index", run_id);
            }
        }

        // 11b. Archive in Xavier (if available)
        if let Some(ref xavier) = self.xavier {
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
                },
                Err(e) => {
                    tracing::warn!("Xavier archive_run failed (non-fatal): {}", e);
                },
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

pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

fn run_git_command(repo_dir: &std::path::Path, args: &[&str]) -> Result<GitOutput, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .map_err(|e| e.to_string())?;

    Ok(GitOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

pub struct SerialMergeQueue {
    pub dry_run: bool,
    pub base_sha: String,
    pub current_commit_or_tree: String,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<ConflictInfo>,
    pub repo_dir: std::path::PathBuf,
    pub binary_mods: HashMap<String, String>, // file_path -> agent_id
}

impl SerialMergeQueue {
    pub fn new(repo_dir: std::path::PathBuf, base_sha: String, dry_run: bool) -> Self {
        Self {
            dry_run,
            base_sha: base_sha.clone(),
            current_commit_or_tree: base_sha,
            merged_branches: Vec::new(),
            conflicts: Vec::new(),
            repo_dir,
            binary_mods: HashMap::new(),
        }
    }

    /// Enqueues and attempts to merge a branch into the current integrated commit/tree.
    /// If successful, updates the current_commit_or_tree and adds to merged_branches.
    /// If it fails due to conflict, rolls back (does not update current_commit_or_tree) and records conflict.
    pub fn enqueue_and_merge(&mut self, agent_id: &str, branch: &str) -> Result<(), RouterError> {
        // 1. Detect binary files modified by this branch
        let args = ["diff", "--numstat", &self.base_sha, branch];
        let git_out = match run_git_command(&self.repo_dir, &args) {
            Ok(out) => out,
            Err(e) => {
                return Err(RouterError::GitError(format!(
                    "Failed to run git diff --numstat for {}: {}",
                    agent_id, e
                )));
            },
        };
        if !git_out.success {
            return Err(RouterError::GitError(format!(
                "Failed to run git diff --numstat for {}: {}",
                agent_id, git_out.stderr
            )));
        }

        let mut branch_binary_mods = Vec::new();
        for line in git_out.stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 && parts[0] == "-" && parts[1] == "-" {
                branch_binary_mods.push(parts[2].to_string());
            }
        }

        // Check for binary conflicts
        let mut has_conflict = false;
        for path in &branch_binary_mods {
            if let Some(_existing_agent) = self.binary_mods.get(path) {
                // Conflict detected!
                self.conflicts.push(ConflictInfo {
                    agent_id: agent_id.to_string(),
                    path: path.clone(),
                });
                has_conflict = true;
            }
        }

        if has_conflict {
            // Auto-rollback/discard this merge: do not update current_commit_or_tree
            return Ok(());
        }

        // 2. Perform merge using git merge-tree
        let merge_args = [
            "merge-tree",
            "--write-tree",
            "--merge-base",
            &self.base_sha,
            &self.current_commit_or_tree,
            branch,
        ];
        let merge_out = match run_git_command(&self.repo_dir, &merge_args) {
            Ok(out) => out,
            Err(e) => {
                return Err(RouterError::GitError(format!(
                    "Failed to execute git merge-tree: {}",
                    e
                )));
            },
        };

        if merge_out.success {
            let merged_tree = merge_out.stdout.trim().to_string();
            if self.dry_run {
                // In dry_run mode, simulate merge without writing any commit object.
                // We just update current_commit_or_tree to the merged tree SHA.
                self.current_commit_or_tree = merged_tree;
                self.merged_branches.push(branch.to_string());
                for path in branch_binary_mods {
                    self.binary_mods.insert(path, agent_id.to_string());
                }
            } else {
                // Create intermediate commit so we have parent references and tree structure
                let commit_args = [
                    "-c",
                    "core.hooksPath=/dev/null",
                    "commit-tree",
                    &merged_tree,
                    "-p",
                    &self.current_commit_or_tree,
                    "-p",
                    branch,
                    "-m",
                    &format!("gestalt: intermediate merge of {}", agent_id),
                ];
                let commit_out = match run_git_command(&self.repo_dir, &commit_args) {
                    Ok(out) => out,
                    Err(e) => {
                        return Err(RouterError::GitError(format!(
                            "Failed to execute git commit-tree: {}",
                            e
                        )));
                    },
                };
                if commit_out.success {
                    self.current_commit_or_tree = commit_out.stdout.trim().to_string();
                    self.merged_branches.push(branch.to_string());
                    for path in branch_binary_mods {
                        self.binary_mods.insert(path, agent_id.to_string());
                    }
                } else {
                    // rollback!
                    self.conflicts.push(ConflictInfo {
                        agent_id: agent_id.to_string(),
                        path: format!("commit-tree-failed: {}", commit_out.stderr),
                    });
                }
            }
        } else {
            // rollback! Parse conflicts from BOTH stdout and stderr
            let err_msg = format!("{}\n{}", merge_out.stdout, merge_out.stderr);
            let mut files = Vec::new();
            for line in err_msg.lines() {
                if line.starts_with("Conflict") || line.contains("conflict") {
                    if let Some(idx) = line.find("in ") {
                        let p = &line[idx + 3..];
                        files.push(p.trim().to_string());
                    } else {
                        let words: Vec<&str> = line.split_whitespace().collect();
                        if !words.is_empty() {
                            files.push(words[words.len() - 1].trim().to_string());
                        }
                    }
                }
            }
            if files.is_empty() {
                files.push(format!("conflict-in-branch-{}", branch));
            }
            files.sort();
            files.dedup();
            for f in files {
                self.conflicts.push(ConflictInfo {
                    agent_id: agent_id.to_string(),
                    path: f,
                });
            }
        }

        Ok(())
    }

    /// Complete integration and return the final merge commit SHA (if not dry-run).
    pub fn finish(self, integration_branch: Option<&str>) -> Result<String, RouterError> {
        if self.dry_run {
            // In dry-run mode, we return an empty string
            Ok(String::new())
        } else {
            let final_sha = self.current_commit_or_tree.clone();

            // Update the local target integration branch to point to final_sha
            let integration_branch_name = integration_branch.unwrap_or("main");
            let ref_args = [
                "update-ref",
                &format!("refs/heads/{}", integration_branch_name),
                &final_sha,
            ];
            let _ = run_git_command(&self.repo_dir, &ref_args);

            Ok(final_sha)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_git_repo(dir: &std::path::Path) -> String {
        let _ = run_git_command(dir, &["init", "-b", "main"]).unwrap();
        let _ = run_git_command(dir, &["config", "user.name", "Test"]).unwrap();
        let _ = run_git_command(dir, &["config", "user.email", "test@example.com"]).unwrap();

        fs::write(dir.join("file1.txt"), "Initial file 1\n").unwrap();
        fs::write(dir.join("file2.txt"), "Initial file 2\n").unwrap();
        let _ = run_git_command(dir, &["add", "."]).unwrap();
        let _ = run_git_command(dir, &["commit", "-m", "initial commit"]).unwrap();

        run_git_command(dir, &["rev-parse", "HEAD"]).unwrap().stdout
    }

    #[test]
    fn test_serial_merge_queue_success() {
        let temp =
            std::env::temp_dir().join(format!("gestalt_test_success_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();

        let base_sha = setup_test_git_repo(&temp);

        // create branch 1
        run_git_command(&temp, &["checkout", "-b", "agent-1-branch"]).unwrap();
        fs::write(temp.join("file1.txt"), "Agent 1 content\n").unwrap();
        run_git_command(&temp, &["commit", "-am", "agent 1 change"]).unwrap();

        // create branch 2
        run_git_command(&temp, &["checkout", "main"]).unwrap();
        run_git_command(&temp, &["checkout", "-b", "agent-2-branch"]).unwrap();
        fs::write(temp.join("file2.txt"), "Agent 2 content\n").unwrap();
        run_git_command(&temp, &["commit", "-am", "agent 2 change"]).unwrap();

        let mut queue = SerialMergeQueue::new(temp.clone(), base_sha, false);
        queue
            .enqueue_and_merge("agent-1", "agent-1-branch")
            .unwrap();
        queue
            .enqueue_and_merge("agent-2", "agent-2-branch")
            .unwrap();

        assert_eq!(queue.merged_branches.len(), 2);
        assert!(queue.conflicts.is_empty());

        let final_sha = queue.finish(None).unwrap();
        assert!(!final_sha.is_empty());

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_serial_merge_queue_rollback_on_conflict() {
        let temp =
            std::env::temp_dir().join(format!("gestalt_test_rollback_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();

        let base_sha = setup_test_git_repo(&temp);

        // create branch 1
        run_git_command(&temp, &["checkout", "-b", "agent-1-branch"]).unwrap();
        fs::write(temp.join("file1.txt"), "Agent 1 conflicting change\n").unwrap();
        run_git_command(&temp, &["commit", "-am", "agent 1 change"]).unwrap();

        // create branch 2
        run_git_command(&temp, &["checkout", "main"]).unwrap();
        run_git_command(&temp, &["checkout", "-b", "agent-2-branch"]).unwrap();
        fs::write(temp.join("file1.txt"), "Agent 2 conflicting change\n").unwrap();
        run_git_command(&temp, &["commit", "-am", "agent 2 change"]).unwrap();

        let mut queue = SerialMergeQueue::new(temp.clone(), base_sha, false);
        queue
            .enqueue_and_merge("agent-1", "agent-1-branch")
            .unwrap();

        let commit_after_agent_1 = queue.current_commit_or_tree.clone();

        queue
            .enqueue_and_merge("agent-2", "agent-2-branch")
            .unwrap();

        assert_eq!(queue.merged_branches.len(), 1);
        assert_eq!(queue.merged_branches[0], "agent-1-branch");
        assert!(!queue.conflicts.is_empty());
        assert_eq!(queue.conflicts[0].agent_id, "agent-2");

        assert_eq!(queue.current_commit_or_tree, commit_after_agent_1);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_serial_merge_queue_dry_run() {
        let temp =
            std::env::temp_dir().join(format!("gestalt_test_dry_run_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();

        let base_sha = setup_test_git_repo(&temp);

        // create branch 1
        run_git_command(&temp, &["checkout", "-b", "agent-1-branch"]).unwrap();
        fs::write(temp.join("file1.txt"), "Agent 1 content\n").unwrap();
        run_git_command(&temp, &["commit", "-am", "agent 1 change"]).unwrap();

        // create branch 2
        run_git_command(&temp, &["checkout", "main"]).unwrap();
        run_git_command(&temp, &["checkout", "-b", "agent-2-branch"]).unwrap();
        fs::write(temp.join("file2.txt"), "Agent 2 content\n").unwrap();
        run_git_command(&temp, &["commit", "-am", "agent 2 change"]).unwrap();

        let mut queue = SerialMergeQueue::new(temp.clone(), base_sha, true);
        queue
            .enqueue_and_merge("agent-1", "agent-1-branch")
            .unwrap();
        queue
            .enqueue_and_merge("agent-2", "agent-2-branch")
            .unwrap();

        assert_eq!(queue.merged_branches.len(), 2);
        assert!(queue.conflicts.is_empty());

        let final_sha = queue.finish(None).unwrap();
        assert!(final_sha.is_empty());

        let _ = fs::remove_dir_all(&temp);
    }
}
