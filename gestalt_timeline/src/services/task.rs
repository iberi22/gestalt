//! Task Service

use anyhow::{Context, Result};

use chrono::{DateTime, Utc};
use std::time::Instant;
use surrealdb::sql::Thing;
use tracing::{debug, info};

use crate::db::SurrealClient;
use crate::models::{EventType, Task, TaskResult, TaskStatus};
use crate::services::TimelineService;

/// Helper to convert Option<Thing> to String
fn thing_to_string(thing: &Option<Thing>) -> String {
    thing.as_ref().map(|t| t.to_string()).unwrap_or_default()
}

/// Service for managing tasks.
#[derive(Clone)]
pub struct TaskService {
    db: SurrealClient,
    timeline: TimelineService,
}

impl TaskService {
    /// Create a new TaskService.
    pub fn new(db: SurrealClient, timeline: TimelineService) -> Self {
        Self { db, timeline }
    }

    /// Create a new task.
    pub async fn create_task(
        &self,
        project_name: &str,
        description: &str,
        agent_id: &str,
    ) -> Result<Task> {
        debug!("Creating task for project: {}", project_name);

        // Find project by name
        let query = "SELECT * FROM projects WHERE name = $name LIMIT 1";
        let projects: Vec<crate::models::Project> = self
            .db
            .query_with::<crate::models::Project>(query, ("name", project_name))
            .await?;

        let project = projects.into_iter().next().context("Project not found")?;

        let project_id_str = thing_to_string(&project.id);
        let task = Task::new(&project_id_str, description, agent_id, None);

        // Store in database
        let created: Task = self.db.create("tasks", &task).await?;

        // Record timeline event
        let task_id_str = thing_to_string(&created.id);
        self.timeline
            .emit_task_event(
                agent_id,
                EventType::TaskCreated,
                &project_id_str,
                &task_id_str,
            )
            .await?;

        Ok(created)
    }

    /// List tasks, optionally filtered by project.
    pub async fn list_tasks(&self, project_name: Option<&str>) -> Result<Vec<Task>> {
        match project_name {
            Some(name) => {
                // Find project first
                let query = "SELECT * FROM projects WHERE name = $name LIMIT 1";
                let projects: Vec<crate::models::Project> = self
                    .db
                    .query_with::<crate::models::Project>(query, ("name", name))
                    .await?;

                if let Some(project) = projects.into_iter().next() {
                    let project_id_str = thing_to_string(&project.id);
                    let query = "SELECT * FROM tasks WHERE project_id = $project_id ORDER BY created_at DESC";
                    self.db
                        .query_with::<Task>(query, ("project_id", &project_id_str))
                        .await
                } else {
                    Ok(vec![])
                }
            }
            None => self.db.select_all("tasks").await,
        }
    }

    /// Get a task by ID.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Task>> {
        self.db.select_by_id("tasks", id).await
    }

    /// Run a task asynchronously.
    ///
    /// This simulates task execution. In a real implementation, this would
    /// dispatch work to actual executors.
    pub async fn run_task(&self, task_id: &str, agent_id: &str) -> Result<TaskResult> {
        info!("Running task: {}", task_id);

        let mut task = self.get_by_id(task_id).await?.context("Task not found")?;

        // Update status to running
        task.status = TaskStatus::Running;
        task.updated_at = crate::models::FlexibleTimestamp::now();
        task.executed_by = Some(agent_id.to_string());
        self.db.update("tasks", task_id, &task).await?;

        // Record start event
        self.timeline
            .emit_task_event(agent_id, EventType::TaskStarted, &task.project_id, task_id)
            .await?;

        // Simulate async work
        let start = Instant::now();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Mark as completed
        let now = crate::models::FlexibleTimestamp::now();
        task.status = TaskStatus::Completed;
        task.completed_at = Some(now.clone());
        task.updated_at = now.clone();
        task.duration_ms = Some(duration_ms);
        self.db.update("tasks", task_id, &task).await?;

        // Record completion event
        self.timeline
            .emit_task_event(
                agent_id,
                EventType::TaskCompleted,
                &task.project_id,
                task_id,
            )
            .await?;

        let result = TaskResult {
            task_id: task_id.to_string(),
            status: TaskStatus::Pending, // Changed from TaskStatus::Completed
            message: format!("Task '{}' completed successfully", task.description),
            duration_ms,
            completed_at: now,
        };

        Ok(result)
    }

    /// Cancel a task.
    pub async fn cancel_task(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let mut task = self.get_by_id(task_id).await?.context("Task not found")?;

        task.status = TaskStatus::Cancelled;
        task.updated_at = crate::models::FlexibleTimestamp::now();
        let updated = self.db.update("tasks", task_id, &task).await?;

        self.timeline
            .emit_task_event(agent_id, EventType::TaskFailed, &task.project_id, task_id)
            .await?;

        Ok(updated)
    }

    /// Update task details.
    pub async fn update_task(
        &self,
        task_id: &str,
        description: Option<String>,
        status: Option<TaskStatus>,
        agent_id: &str,
    ) -> Result<Task> {
        let mut task = self.get_by_id(task_id).await?.context("Task not found")?;

        if let Some(desc) = description {
            task.description = desc;
        }
        if let Some(s) = status {
            if s == TaskStatus::Completed {
                task.completed_at = Some(crate::models::FlexibleTimestamp::now());
            }
            task.status = s;
        }
        task.updated_at = crate::models::FlexibleTimestamp::now();

        let updated = self.db.update("tasks", task_id, &task).await?;

        self.timeline
            .emit_task_event(agent_id, EventType::TaskUpdated, &task.project_id, task_id)
            .await?;

        Ok(updated)
    }

    /// Delete a task.
    pub async fn delete_task(&self, task_id: &str, agent_id: &str) -> Result<()> {
        let task = self.get_by_id(task_id).await?.context("Task not found")?;

        self.db.delete("tasks", task_id).await?;

        self.timeline
            .emit_task_event(agent_id, EventType::TaskDeleted, &task.project_id, task_id)
            .await?;

        Ok(())
    }

    /// Schedule a task.
    pub async fn schedule_task(
        &self,
        task_id: &str,
        execute_at: DateTime<Utc>,
        agent_id: &str,
    ) -> Result<Task> {
        let mut task = self.get_by_id(task_id).await?.context("Task not found")?;

        // In a real system, we would insert this into a scheduling queue.
        // For now, we just update the metadata.
        // Note: Task struct might need a `scheduled_for` field if we want to persist it.
        // For this iteration, we'll just log it and update status.

        info!("Scheduling task {} for {}", task_id, execute_at);
        task.updated_at = crate::models::FlexibleTimestamp::now();

        let updated = self.db.update("tasks", task_id, &task).await?;

        // Emit scheduled event (might need a new EventType, using Updated for now)
        self.timeline
            .emit_task_event(agent_id, EventType::TaskUpdated, &task.project_id, task_id)
            .await?;

        Ok(updated)
    }
}
