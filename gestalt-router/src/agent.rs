use std::path::Path;
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;
use tokio::process::Command;
use crate::run::{AgentSpec, AgentResult, RouterError};
use crate::run_state::AgentState;

pub trait AgentRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        spec: &'a AgentSpec,
        worktree_path: &'a Path,
        task: &'a str,
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<AgentResult, RouterError>> + Send + 'a>>;
}

pub struct SubprocessRunner;

impl SubprocessRunner {
    pub fn new() -> Self {
        Self
    }
}

impl AgentRunner for SubprocessRunner {
    fn run<'a>(
        &'a self,
        spec: &'a AgentSpec,
        worktree_path: &'a Path,
        _task: &'a str,
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<AgentResult, RouterError>> + Send + 'a>> {
        Box::pin(async move {
            let mut cmd = Command::new(&spec.command);
            cmd.args(&spec.args)
                .current_dir(worktree_path)
                .envs(&spec.env)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);

            let child = cmd.spawn()
                .map_err(|e| RouterError::AgentError(format!("Failed to spawn process {}: {}", spec.command, e)))?;

            match tokio::time::timeout(timeout, child.wait_with_output()).await {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                    let state = if output.status.success() {
                        AgentState::Success
                    } else {
                        AgentState::Crashed
                    };
                    Ok(AgentResult {
                        agent_id: spec.id.clone(),
                        state,
                        output: Some(stdout),
                        error: if output.status.success() { None } else { Some(stderr) },
                    })
                }
                Ok(Err(e)) => {
                    Err(RouterError::AgentError(format!("Error waiting for process {}: {}", spec.command, e)))
                }
                Err(_) => {
                    Ok(AgentResult {
                        agent_id: spec.id.clone(),
                        state: AgentState::Timeout,
                        output: None,
                        error: Some("Agent execution timed out".to_string()),
                    })
                }
            }
        })
    }
}
