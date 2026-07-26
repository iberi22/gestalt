use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::agent::AgentRunner;
use crate::run::{AgentResult, RouterError, RunReport, RunSpec};
use crate::run_state::{AgentState, RunManifest};
use crate::timeline::{Event, EventLog};
use crate::worktree::WorktreeManager;

pub struct Router {
    pub worktrees: Arc<WorktreeManager>,
    pub runner: Arc<dyn AgentRunner>,
    pub log: Arc<dyn EventLog>,
}

impl Router {
    pub fn new(
        worktrees: Arc<WorktreeManager>,
        runner: Arc<dyn AgentRunner>,
        log: Arc<dyn EventLog>,
    ) -> Self {
        Self {
            worktrees,
            runner,
            log,
        }
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

    /// Writes the RunManifest atomically to manifest.json.
    pub fn write_manifest_atomically(
        &self,
        run_id: Uuid,
        manifest: &RunManifest,
    ) -> Result<PathBuf, RouterError> {
        let manifest_dir = self.worktrees.base_dir.join(run_id.to_string());
        std::fs::create_dir_all(&manifest_dir).map_err(|e| {
            RouterError::GitError(format!("Failed to create manifest directory: {}", e))
        })?;

        let manifest_path = manifest_dir.join("manifest.json");
        let temp_path = manifest_dir.join("manifest.json.tmp");

        let serialized = serde_json::to_string_pretty(manifest).map_err(|e| {
            RouterError::InvalidSpec(format!("Failed to serialize manifest: {}", e))
        })?;

        std::fs::write(&temp_path, serialized)
            .map_err(|e| RouterError::GitError(format!("Failed to write temp manifest: {}", e)))?;

        std::fs::rename(&temp_path, &manifest_path).map_err(|e| {
            RouterError::GitError(format!("Failed to rename manifest file atomically: {}", e))
        })?;

        Ok(manifest_path)
    }

    /// Main Router pipeline execution.
    pub async fn execute(&self, spec: RunSpec) -> Result<RunReport, RouterError> {
        // 1. Validate spec
        if spec.agents.is_empty() {
            return Err(RouterError::InvalidSpec(
                "No agents specified in RunSpec".to_string(),
            ));
        }

        let base_sha = self.resolve_base_sha(&spec.base_ref)?;

        // Generate Run ID
        let run_id = Uuid::new_v4();

        // 2. Write Manifest file BEFORE creating any resources
        let mut agent_states = HashMap::new();
        for agent in &spec.agents {
            agent_states.insert(agent.id.clone(), AgentState::Pending);
        }

        let manifest = RunManifest {
            run_id,
            spec: spec.clone(),
            agent_states,
        };

        self.write_manifest_atomically(run_id, &manifest)?;

        // 3. Fire RunStarted Event
        let agent_ids: Vec<String> = spec.agents.iter().map(|a| a.id.clone()).collect();
        let _ = self.log.log(Event::RunStarted {
            run_id,
            sha_base: base_sha.clone(),
            agents: agent_ids,
            task: spec.task.clone(),
        });

        // 4. Create Worktrees (serializado)
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

        // 5. Spawn agents in parallel using JoinSet and Semaphore
        let semaphore = Arc::new(Semaphore::new(spec.max_parallel));
        let manifest_mutex = Arc::new(Mutex::new(manifest));
        let mut join_set = JoinSet::new();

        // Share Router via Arc for spawned tasks
        let router = Arc::new(Router {
            worktrees: self.worktrees.clone(),
            runner: self.runner.clone(),
            log: self.log.clone(),
        });

        for agent in spec.agents {
            let sem_clone = semaphore.clone();
            let manifest_lock = manifest_mutex.clone();
            let agent_id = agent.id.clone();
            let agent_spec = agent.clone();
            let task_desc = spec.task.clone();
            let _base_sha_clone = base_sha.clone();
            let run_id_clone = run_id;
            let wt_path = wt_paths.get(&agent_id).cloned().unwrap();
            let timeout = std::time::Duration::from_secs(spec.timeout);
            let router = router.clone();

            join_set.spawn(async move {
                let _permit = sem_clone.acquire_owned().await.unwrap();

                // Transition state to Running
                {
                    let mut m = manifest_lock.lock().await;
                    let old_state = m
                        .agent_states
                        .insert(agent_id.clone(), AgentState::Running)
                        .unwrap_or(AgentState::Pending);
                    router.write_manifest_atomically(run_id_clone, &m)?;
                    let _ = router.log.log(Event::AgentStateChanged {
                        run_id: run_id_clone,
                        agent_id: agent_id.clone(),
                        from: old_state,
                        to: AgentState::Running,
                    });
                }

                // Run Agent
                let mut run_result = router
                    .runner
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

                run_result.state = final_state;

                // Update final state in manifest
                {
                    let mut m = manifest_lock.lock().await;
                    let old_state = m
                        .agent_states
                        .insert(agent_id.clone(), final_state)
                        .unwrap_or(AgentState::Running);
                    router.write_manifest_atomically(run_id_clone, &m)?;
                    let _ = router.log.log(Event::AgentStateChanged {
                        run_id: run_id_clone,
                        agent_id: agent_id.clone(),
                        from: old_state,
                        to: final_state,
                    });
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

        // 6. Overlap detection
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

        let repo_path = std::env::current_dir()
            .map_err(|e| RouterError::GitError(format!("Failed to get current dir: {}", e)))?;
        let overlaps = crate::overlap::find_overlaps(&repo_path, &base_sha, &active_branches)?;
        for overlap in &overlaps {
            let _ = self.log.log(Event::OverlapDetected {
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

        // 7. Sequential branch integration
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
            let _ = self.log.log(Event::MergeConflict {
                run_id,
                agent: conflict.agent_id.clone(),
                path: conflict.path.clone(),
            });
        }

        // 8. Cleanup agent worktrees
        for wt in &created_wts {
            let _ = self.worktrees.cleanup_worktree(wt);
        }

        // 9. Fire RunFinished Event
        let summary = format!(
            "Completed run with {} agents. Merged: {}. Conflicts: {}.",
            agent_results.len(),
            merged_branches.len(),
            conflicts.len()
        );
        let _ = self.log.log(Event::RunFinished { run_id, summary });

        let events_path = self
            .worktrees
            .base_dir
            .join(run_id.to_string())
            .join("events.jsonl")
            .to_string_lossy()
            .to_string();

        Ok(RunReport {
            run_id,
            agents: agent_results,
            merged_branches,
            conflicts,
            events_path,
            success: true,
        })
    }
}
