use crate::run::{AgentResult, AgentSpec, RouterError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutcome {
    pub state: crate::run_state::AgentState,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub duration: Duration,
    pub files_changed: Vec<PathBuf>,
}

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(
        &self,
        spec: &AgentSpec,
        worktree: &Path,
        task: &str,
        timeout: Duration,
    ) -> Result<AgentResult, RouterError>;
}

pub struct SubprocessRunner {
    pub timeout: Duration,
}

impl SubprocessRunner {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl AgentRunner for SubprocessRunner {
    async fn run(
        &self,
        spec: &AgentSpec,
        worktree: &Path,
        task: &str,
        timeout: Duration,
    ) -> Result<AgentResult, RouterError> {
        let start_time = Instant::now();

        // 1. Generate unique stdout and stderr file paths
        let run_id = uuid::Uuid::new_v4();
        let stdout_path =
            std::env::temp_dir().join(format!("agent_stdout_{}_{}.log", spec.id, run_id));
        let stderr_path =
            std::env::temp_dir().join(format!("agent_stderr_{}_{}.log", spec.id, run_id));

        // Create the files
        let mut stdout_file = tokio::fs::File::create(&stdout_path)
            .await
            .map_err(|e| RouterError::AgentError(format!("Failed to create stdout file: {}", e)))?;
        let mut stderr_file = tokio::fs::File::create(&stderr_path)
            .await
            .map_err(|e| RouterError::AgentError(format!("Failed to create stderr file: {}", e)))?;

        // 2. Build the Command
        let mut cmd = tokio::process::Command::new(&spec.command);
        cmd.args(&spec.args);
        cmd.current_dir(worktree);

        // Sanitize environment variables: clear everything, then add safe essential variables plus user-specified ones.
        cmd.env_clear();
        let mut has_path = false;
        for var in &["PATH", "HOME", "USER", "TERM", "LANG", "LC_ALL"] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
                if *var == "PATH" {
                    has_path = true;
                }
            }
        }
        if !has_path {
            cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
        }

        if let Some(ref env) = spec.env {
            for (k, v) in env.iter() {
                cmd.env(k, v);
            }
        }

        // Add the task itself as an environment variable or context if needed, but the main thing is sanitizing env
        cmd.env("GESTALT_TASK", task);

        // Configure process group on Unix (safe, no unsafe)
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // 3. Spawn the child process
        let mut child = cmd.spawn().map_err(|e| {
            RouterError::AgentError(format!("Failed to spawn agent process: {}", e))
        })?;

        // Extract pid immediately to avoid borrow issues later
        let pid = child.id();

        // Extract stdout/stderr pipes
        let mut child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| RouterError::AgentError("Failed to open stdout pipe".to_string()))?;
        let mut child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| RouterError::AgentError("Failed to open stderr pipe".to_string()))?;

        // Copy stdout/stderr to files concurrently
        let stdout_handle =
            tokio::spawn(async move { tokio::io::copy(&mut child_stdout, &mut stdout_file).await });
        let stderr_handle =
            tokio::spawn(async move { tokio::io::copy(&mut child_stderr, &mut stderr_file).await });

        // 4. Wait with timeout and graceful termination/SIGKILL pattern
        let wait_fut = child.wait();
        tokio::pin!(wait_fut);

        let mut exit_code = None;
        let mut is_timeout = false;

        match tokio::time::timeout(timeout, &mut wait_fut).await {
            Ok(Ok(status)) => {
                exit_code = status.code();
            }
            Ok(Err(e)) => {
                return Err(RouterError::AgentError(format!(
                    "Process wait error: {}",
                    e
                )));
            }
            Err(_) => {
                // Timeout occurred!
                is_timeout = true;

                // Send SIGTERM to the process group (using negative pgid)
                #[cfg(unix)]
                {
                    if let Some(p) = pid {
                        if p > 1 {
                            unsafe {
                                libc::kill(-(p as libc::pid_t), libc::SIGTERM);
                            }
                        }
                    }
                }

                // Wait up to 5 seconds for the process to exit gracefully
                let grace_duration = Duration::from_secs(5);
                match tokio::time::timeout(grace_duration, &mut wait_fut).await {
                    Ok(Ok(_status)) => {}
                    Ok(Err(e)) => {
                        return Err(RouterError::AgentError(format!(
                            "Process wait error after SIGTERM: {}",
                            e
                        )));
                    }
                    Err(_) => {
                        // Grace period expired! Send SIGKILL to the process group
                        #[cfg(unix)]
                        {
                            if let Some(p) = pid {
                                if p > 1 {
                                    unsafe {
                                        libc::kill(-(p as libc::pid_t), libc::SIGKILL);
                                    }
                                }
                            }
                        }

                        // Final wait to reap the process
                        if let Err(e) = wait_fut.await {
                            return Err(RouterError::AgentError(format!(
                                "Process reap error after SIGKILL: {}",
                                e
                            )));
                        }
                    }
                }
            }
        }

        // Wait for stdout and stderr copying tasks to finish
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        // Cleanup process group to kill orphan grandchildren
        #[cfg(unix)]
        {
            if let Some(p) = pid {
                if p > 1 {
                    unsafe {
                        libc::kill(-(p as libc::pid_t), libc::SIGKILL);
                    }
                }
            }
        }

        let duration = start_time.elapsed();

        if is_timeout {
            exit_code = Some(-1);
        }

        // Read captured output and delete temporary files to avoid disk leaks
        let stdout_content = tokio::fs::read_to_string(&stdout_path).await.unwrap_or_default();
        let stderr_content = tokio::fs::read_to_string(&stderr_path).await.unwrap_or_default();
        let _ = tokio::fs::remove_file(&stdout_path).await;
        let _ = tokio::fs::remove_file(&stderr_path).await;

        let mut full_output = String::new();
        if !stdout_content.is_empty() {
            full_output.push_str(&stdout_content);
        }
        if !stderr_content.is_empty() {
            if !full_output.is_empty() {
                full_output.push('\n');
            }
            full_output.push_str(&stderr_content);
        }

        let output_opt = if full_output.is_empty() {
            None
        } else {
            Some(full_output)
        };

        // 5. Gather files changed via git status --porcelain
        let mut files_changed = Vec::new();
        let mut git_cmd = tokio::process::Command::new("git");
        git_cmd
            .arg("status")
            .arg("--porcelain")
            .current_dir(worktree);
        if let Ok(output) = git_cmd.output().await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.len() > 3 {
                        let file_path = line[3..].trim_matches('"');
                        files_changed.push(worktree.join(file_path));
                    }
                }
            }
        }

        let state = if is_timeout {
            crate::run_state::AgentState::Timeout
        } else if exit_code.map_or(false, |c| c != 0) {
            crate::run_state::AgentState::Crashed
        } else {
            crate::run_state::AgentState::Success
        };

        let error = if is_timeout {
            Some("Agent process timed out".to_string())
        } else if exit_code.map_or(false, |c| c != 0) {
            Some(format!("Process exited with non-zero code: {:?}", exit_code))
        } else {
            None
        };

        Ok(crate::run::AgentResult {
            agent_id: spec.id.clone(),
            state,
            output: output_opt,
            error,
            branch: None,
            changed_files: files_changed
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            duration_ms: duration.as_millis() as u64,
            run_id: None,
            worktree_path: Some(worktree.to_string_lossy().to_string()),
        })
    }
}
