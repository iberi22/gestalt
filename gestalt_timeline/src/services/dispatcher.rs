//! Subprocess Dispatcher Service
//!
//! Manages headless execution of external CLI agents (like codex, gemini, claude)
//! and captures their output into the Universal Timeline.

use crate::models::EventType;
use crate::services::TimelineService;
use anyhow::Result;
use std::process::Stdio;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{error, info};

/// Background dispatcher for CLI agents
#[derive(Clone)]
pub struct DispatcherService {
    timeline: Arc<TimelineService>,
}

impl DispatcherService {
    pub fn new(timeline: Arc<TimelineService>) -> Self {
        Self { timeline }
    }

    /// Spawn a headless background agent
    pub async fn spawn_agent(&self, agent_type: &str, prompt: &str) -> Result<String> {
        let task_name = format!("subagent-{}", uuid::Uuid::new_v4());
        let agent_type_str = agent_type.to_string();
        let prompt_str = prompt.to_string();

        info!(
            "Spawning background CLI agent: {} '{}'",
            agent_type_str, prompt_str
        );

        // Record spawn event
        self.timeline
            .emit(
                &agent_type_str,
                EventType::SubAgentSpawned(format!("{} {}", agent_type_str, prompt_str)),
            )
            .await?;

        let tl_clone = self.timeline.clone();
        let log_filename = format!("{}.log", task_name);

        tokio::spawn(async move {
            let mut cmd = Command::new(&agent_type_str);
            cmd.arg(&prompt_str);
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take().expect("Failed to open stdout");
                    let stderr = child.stderr.take().expect("Failed to open stderr");

                    let mut stdout_reader = BufReader::new(stdout).lines();
                    let mut stderr_reader = BufReader::new(stderr).lines();

                    let tl_inner = tl_clone.clone();
                    let a_type_inner = agent_type_str.clone();

                    let mut log_file = match File::create(&log_filename).await {
                        Ok(f) => Some(f),
                        Err(e) => {
                            error!("Failed to create log file {}: {}", log_filename, e);
                            None
                        }
                    };

                    tokio::spawn(async move {
                        let mut stdout_done = false;
                        let mut stderr_done = false;

                        loop {
                            tokio::select! {
                                result = stdout_reader.next_line(), if !stdout_done => {
                                    match result {
                                        Ok(Some(line)) => {
                                            if let Some(ref mut f) = log_file {
                                                let _ = f.write_all(format!("[STDOUT] {}\n", line).as_bytes()).await;
                                            }
                                            let _ = tl_inner
                                                .emit(&a_type_inner, EventType::SubAgentOutput(line))
                                                .await;
                                        }
                                        _ => stdout_done = true,
                                    }
                                }
                                result = stderr_reader.next_line(), if !stderr_done => {
                                    match result {
                                        Ok(Some(line)) => {
                                            if let Some(ref mut f) = log_file {
                                                let _ = f.write_all(format!("[STDERR] {}\n", line).as_bytes()).await;
                                            }
                                            let _ = tl_inner
                                                .emit(&a_type_inner, EventType::SubAgentOutput(format!("[STDERR] {}", line)))
                                                .await;
                                        }
                                        _ => stderr_done = true,
                                    }
                                }
                            }

                            if stdout_done && stderr_done {
                                break;
                            }
                        }
                    });

                    match child.wait().await {
                        Ok(status) if status.success() => {
                            let _ = tl_clone
                                .emit(
                                    &agent_type_str,
                                    EventType::SubAgentCompleted(format!("Exited with {}", status)),
                                )
                                .await;
                        }
                        Ok(status) => {
                            let _ = tl_clone
                                .emit(
                                    &agent_type_str,
                                    EventType::SubAgentFailed(format!(
                                        "Exited with error: {}",
                                        status
                                    )),
                                )
                                .await;
                        }
                        Err(e) => {
                            let _ = tl_clone
                                .emit(
                                    &agent_type_str,
                                    EventType::SubAgentFailed(format!(
                                        "Execution context failed: {}",
                                        e
                                    )),
                                )
                                .await;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to spawn {}: {}", agent_type_str, e);
                    let _ = tl_clone
                        .emit(
                            &agent_type_str,
                            EventType::SubAgentFailed(format!("Spawn failed: {}", e)),
                        )
                        .await;
                }
            }
        });

        Ok(task_name)
    }
}
