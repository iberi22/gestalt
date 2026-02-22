//! TaskQueue - Multi-source task ingestion and dispatch.
//!
//! Accepts tasks from: Telegram, CLI, REST API, webhooks, Cron.
//! Persists them to SurrealDB and dispatches to available AgentRuntime workers.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use chrono::Utc;

use crate::db::SurrealClient;

/// Where a task originated from
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskSource {
    Telegram { user_id: String, chat_id: String },
    Cli { invocation: String },
    Api { endpoint: String },
    Cron { schedule: String },
    Agent { parent_id: String },
}

/// A task queued for execution by the agentic system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    pub id: Option<surrealdb::sql::Thing>,
    /// Natural language description of the goal
    pub goal: String,
    /// Where this task came from
    pub source: TaskSource,
    /// Priority (higher = executed sooner, default 5)
    pub priority: u8,
    /// When this was queued
    pub queued_at: chrono::DateTime<Utc>,
    /// Processing state
    pub status: QueueStatus,
    /// Agent assigned to run this task (set when dispatched)
    pub assigned_agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueueStatus {
    Pending,
    Running,
    Completed,
    Failed { reason: String },
}

impl QueuedTask {
    pub fn new(goal: impl Into<String>, source: TaskSource, priority: u8) -> Self {
        Self {
            id: None,
            goal: goal.into(),
            source,
            priority: priority.clamp(1, 10),
            queued_at: Utc::now(),
            status: QueueStatus::Pending,
            assigned_agent_id: None,
        }
    }
}

/// The TaskQueue receives tasks from all ingestion sources and dispatches them
/// to `AgentRuntime` workers via mpsc channels.
#[derive(Clone)]
pub struct TaskQueue {
    db: SurrealClient,
    /// Channel sender — any ingestion source can clone and push tasks here
    sender: mpsc::Sender<QueuedTask>,
}

impl TaskQueue {
    /// Create a new TaskQueue. Returns (queue, receiver).
    /// The caller is responsible for running the dispatch loop with the receiver.
    pub fn new(db: SurrealClient, buffer: usize) -> (Self, mpsc::Receiver<QueuedTask>) {
        let (sender, receiver) = mpsc::channel(buffer);
        let queue = Self {
            db,
            sender,
        };
        (queue, receiver)
    }

    /// Enqueue a task — thread-safe, can be called from any ingestion source.
    pub async fn enqueue(&self, task: QueuedTask) -> Result<()> {
        info!("📥 [TaskQueue] Enqueuing task: '{}' (source: {:?}, priority: {})",
              &task.goal[..task.goal.len().min(80)], task.source, task.priority);

        // Persist to SurrealDB for durability
        let _saved = self.db
            .create("task_queue", &task)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to persist queued task: {}", e))?;

        // Send to dispatch channel
        self.sender.send(task).await
            .map_err(|e| anyhow::anyhow!("TaskQueue channel closed: {}", e))?;

        Ok(())
    }

    /// Create a clone of the sender for use in ingestion sources (Telegram, API, etc.)
    pub fn sender(&self) -> mpsc::Sender<QueuedTask> {
        self.sender.clone()
    }

    /// Load any pending tasks from SurrealDB (useful after restart).
    pub async fn recover_pending(&self) -> Result<Vec<QueuedTask>> {
        let pending: Vec<QueuedTask> = self.db
            .query_with("SELECT * FROM task_queue WHERE status = 'Pending' ORDER BY priority DESC, queued_at ASC LIMIT 100", serde_json::json!({}))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to recover pending tasks: {}", e))?;

        info!("🔄 [TaskQueue] Recovered {} pending tasks from DB", pending.len());
        Ok(pending)
    }

    /// Start the dispatch loop. Reads tasks from the channel and runs AgentRuntime for each.
    /// This is the main always-on worker loop.
    ///
    /// `make_runtime` is a factory closure that creates a fresh `AgentRuntime` for each task.
    pub async fn run_dispatch_loop<F, Fut>(
        &self,
        mut receiver: mpsc::Receiver<QueuedTask>,
        max_concurrent: usize,
        make_runtime: F,
    ) where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<crate::services::AgentRuntime>> + Send + 'static,
    {
        info!("🚦 [TaskQueue] Dispatch loop started (max_concurrent={})", max_concurrent);

        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        let db = self.db.clone();

        while let Some(task) = receiver.recv().await {
            let goal = task.goal.clone();
            let task_id = task.id.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "unknown".to_string());
            let sem = semaphore.clone();
            let factory = make_runtime.clone();
            let db_clone = db.clone();

            info!("🎯 [TaskQueue] Dispatching task '{}' (id={})", &goal[..goal.len().min(60)], task_id);

            tokio::spawn(async move {
                // Acquire semaphore slot (limits concurrency)
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(e) => { error!("Semaphore error: {}", e); return; }
                };

                // Build agent runtime for this task
                let agent_id = format!("worker-{}", chrono::Utc::now().timestamp_millis());
                match factory(agent_id.clone()).await {
                    Ok(runtime) => {
                        info!("🤖 [TaskQueue] Agent '{}' starting goal: {}", agent_id, &goal[..goal.len().min(60)]);
                        match runtime.run_loop(&goal).await {
                            Ok(_) => {
                                info!("✅ [TaskQueue] Agent '{}' completed goal.", agent_id);
                                let _ = db_clone.query_with::<serde_json::Value>(
                                    "UPDATE task_queue SET status = 'Completed' WHERE id = $id",
                                    serde_json::json!({ "id": task_id }),
                                ).await;
                            }
                            Err(e) => {
                                warn!("❌ [TaskQueue] Agent '{}' failed: {}", agent_id, e);
                                let _ = db_clone.query_with::<serde_json::Value>(
                                    "UPDATE task_queue SET status = $status WHERE id = $id",
                                    serde_json::json!({ "status": format!("{{\"Failed\":{{\"reason\":\"{}\"}}}}", e), "id": task_id }),
                                ).await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to create AgentRuntime for task '{}': {}", goal, e);
                    }
                }
            });
        }

        warn!("⚠️ [TaskQueue] Dispatch loop ended — receiver channel closed.");
    }
}
