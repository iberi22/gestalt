use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use crate::run::{AgentSpec, RouterError};
use serde::{Serialize, Deserialize};

pub trait EventLog: Send + Sync {
    // Basic placeholder trait as specified.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutcome {
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
        log: &dyn EventLog,
    ) -> Result<AgentOutcome, RouterError>;
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
        _log: &dyn EventLog,
    ) -> Result<AgentOutcome, RouterError> {
        let start_time = Instant::now();

        // 1. Generate unique stdout and stderr file paths
        let run_id = uuid::Uuid::new_v4();
        let stdout_path = std::env::temp_dir().join(format!("agent_stdout_{}_{}.log", spec.id, run_id));
        let stderr_path = std::env::temp_dir().join(format!("agent_stderr_{}_{}.log", spec.id, run_id));

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
        for var in &["PATH", "HOME", "USER", "TERM", "LANG", "LC_ALL"] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        // Add the task itself as an environment variable or context if needed, but the main thing is sanitizing env
        cmd.env("GESTALT_TASK", task);

        // Configure process group setsid on Unix
        #[cfg(unix)]
        {
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // 3. Spawn the child process
        let mut child = cmd
            .spawn()
            .map_err(|e| RouterError::AgentError(format!("Failed to spawn agent process: {}", e)))?;

        // Extract pid immediately to avoid borrow issues later
        let pid = child.id();

        // Extract stdout/stderr pipes
        let mut child_stdout = child.stdout.take().ok_or_else(|| {
            RouterError::AgentError("Failed to open stdout pipe".to_string())
        })?;
        let mut child_stderr = child.stderr.take().ok_or_else(|| {
            RouterError::AgentError("Failed to open stderr pipe".to_string())
        })?;

        // Copy stdout/stderr to files concurrently
        let stdout_handle = tokio::spawn(async move {
            tokio::io::copy(&mut child_stdout, &mut stdout_file).await
        });
        let stderr_handle = tokio::spawn(async move {
            tokio::io::copy(&mut child_stderr, &mut stderr_file).await
        });

        // 4. Wait with timeout and graceful termination/SIGKILL pattern
        let wait_fut = child.wait();
        tokio::pin!(wait_fut);

        let mut exit_code = None;
        let mut is_timeout = false;

        match tokio::time::timeout(self.timeout, &mut wait_fut).await {
            Ok(Ok(status)) => {
                exit_code = status.code();
            }
            Ok(Err(e)) => {
                return Err(RouterError::AgentError(format!("Process wait error: {}", e)));
            }
            Err(_) => {
                // Timeout occurred!
                is_timeout = true;

                // Send SIGTERM to the process group (using negative pgid)
                #[cfg(unix)]
                {
                    if let Some(p) = pid {
                        unsafe {
                            libc::kill(-(p as libc::pid_t), libc::SIGTERM);
                        }
                    }
                }

                // Wait up to 5 seconds for the process to exit gracefully
                let grace_duration = Duration::from_secs(5);
                match tokio::time::timeout(grace_duration, &mut wait_fut).await {
                    Ok(Ok(_status)) => {}
                    Ok(Err(e)) => {
                        return Err(RouterError::AgentError(format!("Process wait error after SIGTERM: {}", e)));
                    }
                    Err(_) => {
                        // Grace period expired! Send SIGKILL to the process group
                        #[cfg(unix)]
                        {
                            if let Some(p) = pid {
                                unsafe {
                                    libc::kill(-(p as libc::pid_t), libc::SIGKILL);
                                }
                            }
                        }

                        // Final wait to reap the process
                        if let Err(e) = wait_fut.await {
                            return Err(RouterError::AgentError(format!("Process reap error after SIGKILL: {}", e)));
                        }
                    }
                }
            }
        }

        // Wait for stdout and stderr copying tasks to finish
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        let duration = start_time.elapsed();

        if is_timeout {
            exit_code = Some(-1);
        }

        // 5. Gather files changed via git status --porcelain
        let mut files_changed = Vec::new();
        let mut git_cmd = tokio::process::Command::new("git");
        git_cmd.arg("status").arg("--porcelain").current_dir(worktree);
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

        Ok(AgentOutcome {
            exit_code,
            stdout_path,
            stderr_path,
            duration,
            files_changed,
        })
    }
}
