//! Subprocess Dispatcher Service
//!
//! Manages headless execution of external CLI agents (like codex, gemini, claude)
//! and captures their output into the Universal Timeline.

use tokio::process::Command;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, error};
use anyhow::Result;
use std::sync::Arc;
use crate::services::TimelineService;
use crate::models::EventType;

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

        info!("Spawning background CLI agent: {} '{}'", agent_type_str, prompt_str);

        // Record spawn event
        self.timeline.emit(
            &agent_type_str,
            EventType::SubAgentSpawned(format!("{} {}", agent_type_str, prompt_str))
        ).await?;

        let tl_clone = self.timeline.clone();

        tokio::spawn(async move {
            let mut cmd = Command::new(&agent_type_str);
            cmd.arg(&prompt_str);
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take().expect("Failed to open stdout");
                    let mut reader = BufReader::new(stdout).lines();

                    let tl_inner = tl_clone.clone();
                    let a_type_inner = agent_type_str.clone();
                    tokio::spawn(async move {
                        while let Ok(Some(line)) = reader.next_line().await {
                            let _ = tl_inner.emit(
                                &a_type_inner,
                                EventType::SubAgentOutput(line)
                            ).await;
                        }
                    });

                    match child.wait().await {
                        Ok(status) if status.success() => {
                            let _ = tl_clone.emit(
                                &agent_type_str,
                                EventType::SubAgentCompleted(format!("Exited with {}", status))
                            ).await;
                        }
                        Ok(status) => {
                            let _ = tl_clone.emit(
                                &agent_type_str,
                                EventType::SubAgentFailed(format!("Exited with error: {}", status))
                            ).await;
                        }
                        Err(e) => {
                            let _ = tl_clone.emit(
                                &agent_type_str,
                                EventType::SubAgentFailed(format!("Execution context failed: {}", e))
                            ).await;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to spawn {}: {}", agent_type_str, e);
                    let _ = tl_clone.emit(
                        &agent_type_str,
                        EventType::SubAgentFailed(format!("Spawn failed: {}", e))
                    ).await;
                }
            }
        });

        Ok(task_name)
    }
}
