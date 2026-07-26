use crate::run::{AgentResult, AgentSpec, RouterError, RunReport, RunSpec};
use crate::run_state::{AgentState, RunManifest};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug)]
struct RunInner {
    status: RunStatus,
    manifest: RunManifest,
    result: Option<Result<RunReport, RouterError>>,
}

#[derive(Debug, Clone)]
pub struct RunHandle {
    run_id: uuid::Uuid,
    cancel_token: CancellationToken,
    inner: Arc<std::sync::Mutex<RunInner>>,
    completed_rx: tokio::sync::watch::Receiver<bool>,
}

impl RunHandle {
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn status(&self) -> RunStatus {
        self.inner.lock().unwrap().status
    }

    pub fn run_id(&self) -> uuid::Uuid {
        self.run_id
    }

    pub fn manifest(&self) -> RunManifest {
        self.inner.lock().unwrap().manifest.clone()
    }

    pub async fn await_completion(&self) -> Result<RunReport, RouterError> {
        let mut rx = self.completed_rx.clone();
        while !*rx.borrow() {
            if rx.changed().await.is_err() {
                break;
            }
        }

        let inner = self.inner.lock().unwrap();
        match &inner.result {
            Some(Ok(report)) => Ok(report.clone()),
            Some(Err(err)) => Err(clone_router_error(err)),
            None => Err(RouterError::AgentError(
                "Execution finished but no result found".to_string(),
            )),
        }
    }
}

#[derive(Debug, Default)]
pub struct ProcessManager;

impl ProcessManager {
    pub fn new() -> Self {
        Self
    }

    pub fn start_run(&self, spec: RunSpec) -> RunHandle {
        let run_id = uuid::Uuid::new_v4();
        let cancel_token = CancellationToken::new();

        let mut agent_states = std::collections::HashMap::new();
        for agent in &spec.agents {
            agent_states.insert(agent.id.clone(), AgentState::Pending);
        }

        let manifest = RunManifest {
            run_id,
            spec: spec.clone(),
            agent_states,
        };

        let inner = Arc::new(std::sync::Mutex::new(RunInner {
            status: RunStatus::Pending,
            manifest,
            result: None,
        }));

        let (completed_tx, completed_rx) = tokio::sync::watch::channel(false);

        let inner_clone = inner.clone();
        let cancel_token_clone = cancel_token.clone();
        let spec_clone = spec.clone();

        tokio::spawn(async move {
            // Transition status to Running
            {
                let mut lock = inner_clone.lock().unwrap();
                lock.status = RunStatus::Running;
            }

            let timeout_duration = std::time::Duration::from_secs(spec_clone.timeout);

            // Execute under timeout if timeout > 0
            let run_result = if spec_clone.timeout > 0 {
                match tokio::time::timeout(
                    timeout_duration,
                    run_agents_workflow(spec_clone, cancel_token_clone.clone(), inner_clone.clone()),
                )
                .await
                {
                    Ok(res) => res,
                    Err(_) => {
                        // Global Timeout occurred!
                        cancel_token_clone.cancel();
                        {
                            let mut lock = inner_clone.lock().unwrap();
                            lock.status = RunStatus::Failed;
                            // Set all pending or running agents to Timeout
                            for state in lock.manifest.agent_states.values_mut() {
                                if *state == AgentState::Pending || *state == AgentState::Running {
                                    *state = AgentState::Timeout;
                                }
                            }
                        }
                        Err(RouterError::Timeout)
                    }
                }
            } else {
                run_agents_workflow(spec_clone, cancel_token_clone.clone(), inner_clone.clone()).await
            };

            // Post-execution: update status and final result
            {
                let mut lock = inner_clone.lock().unwrap();
                match run_result {
                    Ok(report) => {
                        lock.status = RunStatus::Completed;
                        lock.result = Some(Ok(report));
                    }
                    Err(err) => {
                        if matches!(lock.status, RunStatus::Failed) {
                            // If it already timed out and marked as Failed, keep that
                            lock.result = Some(Err(err));
                        } else if cancel_token_clone.is_cancelled() {
                            lock.status = RunStatus::Cancelled;
                            lock.result = Some(Err(RouterError::AgentError("Run cancelled".to_string())));
                            // Set all pending or running agents to Crashed
                            for state in lock.manifest.agent_states.values_mut() {
                                if *state == AgentState::Pending || *state == AgentState::Running {
                                    *state = AgentState::Crashed;
                                }
                            }
                        } else {
                            lock.status = RunStatus::Failed;
                            lock.result = Some(Err(err));
                        }
                    }
                }
            }

            // Signal completion to all waiters
            let _ = completed_tx.send(true);
        });

        RunHandle {
            run_id,
            cancel_token,
            inner,
            completed_rx,
        }
    }
}

async fn run_agents_workflow(
    spec: RunSpec,
    cancel_token: CancellationToken,
    inner: Arc<std::sync::Mutex<RunInner>>,
) -> Result<RunReport, RouterError> {
    let max_parallel = if spec.max_parallel > 0 {
        spec.max_parallel
    } else {
        1
    };
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_parallel));
    let mut futures = vec![];

    for agent_spec in spec.agents {
        let sem = semaphore.clone();
        let cancel_token_agent = cancel_token.child_token();
        let inner_agent = inner.clone();

        let handle = tokio::spawn(async move {
            // Acquire permit to respect max_parallel
            let _permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return AgentResult {
                        agent_id: agent_spec.id.clone(),
                        state: AgentState::Crashed,
                        output: None,
                        error: Some("Semaphore closed".to_string()),
                    };
                }
            };

            // Update agent state to Running (if not already Timeout)
            {
                let mut lock = inner_agent.lock().unwrap();
                if let Some(state) = lock.manifest.agent_states.get_mut(&agent_spec.id) {
                    if *state == AgentState::Timeout {
                        return AgentResult {
                            agent_id: agent_spec.id.clone(),
                            state: AgentState::Timeout,
                            output: None,
                            error: Some("Agent timed out before starting".to_string()),
                        };
                    }
                    *state = AgentState::Running;
                }
            }

            // Check cancellation before executing
            if cancel_token_agent.is_cancelled() {
                let mut lock = inner_agent.lock().unwrap();
                let is_timeout = if let Some(state) = lock.manifest.agent_states.get_mut(&agent_spec.id) {
                    if *state == AgentState::Timeout {
                        true
                    } else {
                        *state = AgentState::Crashed;
                        false
                    }
                } else {
                    false
                };

                return AgentResult {
                    agent_id: agent_spec.id.clone(),
                    state: if is_timeout { AgentState::Timeout } else { AgentState::Crashed },
                    output: None,
                    error: Some("Agent execution cancelled".to_string()),
                };
            }

            // Execute the agent command (or simulated command)
            let result = execute_single_agent(&agent_spec, &cancel_token_agent).await;

            // Update agent state to final state
            {
                let mut lock = inner_agent.lock().unwrap();
                if let Some(state) = lock.manifest.agent_states.get_mut(&agent_spec.id) {
                    if *state != AgentState::Timeout {
                        *state = result.state;
                    }
                }
            }

            result
        });
        futures.push(handle);
    }

    // Wait for all agent tasks to complete
    let mut agent_results = vec![];
    for fut in futures {
        match fut.await {
            Ok(res) => {
                agent_results.push(res);
            }
            Err(e) => {
                return Err(RouterError::AgentError(format!("Task join error: {}", e)));
            }
        }
    }

    // Check if the run was cancelled globally while executing agents
    if cancel_token.is_cancelled() {
        return Err(RouterError::AgentError("Run cancelled".to_string()));
    }

    let run_id = inner.lock().unwrap().manifest.run_id;
    Ok(RunReport {
        run_id,
        task: spec.task,
        agents: agent_results,
        duration_ms: 0,
        merged_branches: vec![],
        conflicts: vec![],
        events_path: "".to_string(),
        success: true,
    })
}

async fn execute_single_agent(
    agent_spec: &AgentSpec,
    cancel_token: &CancellationToken,
) -> AgentResult {
    match agent_spec.command.as_str() {
        "mock_success" => {
            let msg = agent_spec.args.first().cloned().unwrap_or_else(|| "Success".to_string());
            AgentResult {
                agent_id: agent_spec.id.clone(),
                state: AgentState::Success,
                output: Some(msg),
                error: None,
            }
        }
        "mock_fail" => {
            let msg = agent_spec.args.first().cloned().unwrap_or_else(|| "Failure".to_string());
            AgentResult {
                agent_id: agent_spec.id.clone(),
                state: AgentState::Crashed,
                output: None,
                error: Some(msg),
            }
        }
        "mock_sleep" => {
            let sleep_ms: u64 = agent_spec
                .args
                .first()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000);

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)) => {
                    AgentResult {
                        agent_id: agent_spec.id.clone(),
                        state: AgentState::Success,
                        output: Some("Sleep finished".to_string()),
                        error: None,
                    }
                }
                _ = cancel_token.cancelled() => {
                    AgentResult {
                        agent_id: agent_spec.id.clone(),
                        state: AgentState::Crashed,
                        output: None,
                        error: Some("Agent execution cancelled".to_string()),
                    }
                }
            }
        }
        _ => {
            let mut cmd = tokio::process::Command::new(&agent_spec.command);
            cmd.args(&agent_spec.args);
            for (k, v) in &agent_spec.env {
                cmd.env(k, v);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    return AgentResult {
                        agent_id: agent_spec.id.clone(),
                        state: AgentState::Crashed,
                        output: None,
                        error: Some(format!("Failed to spawn command: {}", e)),
                    };
                }
            };

            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();

            let mut stdout_bytes = Vec::new();
            let mut stderr_bytes = Vec::new();

            let read_stdout = tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut stdout_bytes);
            let read_stderr = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_bytes);
            let wait_child = child.wait();

            tokio::select! {
                res = async {
                    tokio::try_join!(read_stdout, read_stderr, wait_child)
                } => {
                    match res {
                        Ok((_, _, exit_status)) => {
                            let stdout_str = String::from_utf8_lossy(&stdout_bytes).into_owned();
                            let stderr_str = String::from_utf8_lossy(&stderr_bytes).into_owned();

                            let state = if exit_status.success() {
                                AgentState::Success
                            } else {
                                AgentState::Crashed
                            };

                            AgentResult {
                                agent_id: agent_spec.id.clone(),
                                state,
                                output: Some(stdout_str),
                                error: Some(stderr_str),
                            }
                        }
                        Err(e) => {
                            AgentResult {
                                agent_id: agent_spec.id.clone(),
                                state: AgentState::Crashed,
                                output: None,
                                error: Some(format!("Error reading process streams: {}", e)),
                            }
                        }
                    }
                }
                _ = cancel_token.cancelled() => {
                    let _ = child.kill().await;
                    AgentResult {
                        agent_id: agent_spec.id.clone(),
                        state: AgentState::Crashed,
                        output: None,
                        error: Some("Agent execution cancelled".to_string()),
                    }
                }
            }
        }
    }
}

fn clone_router_error(err: &RouterError) -> RouterError {
    match err {
        RouterError::GitError(s) => RouterError::GitError(s.clone()),
        RouterError::AgentError(s) => RouterError::AgentError(s.clone()),
        RouterError::Timeout => RouterError::Timeout,
        RouterError::InvalidSpec(s) => RouterError::InvalidSpec(s.clone()),
    }
}
