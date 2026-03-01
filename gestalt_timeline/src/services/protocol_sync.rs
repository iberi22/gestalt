//! Protocol Synchronization Service
//!
//! Synchronizes SurrealDB states with local markdown files (e.g. TASK.md).

use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;
use crate::models::{Task, TaskStatus};
use crate::db::SurrealClient;
use crate::services::TimelineService;
use crate::models::EventType;

pub struct ProtocolSyncService {
    db: SurrealClient,
    timeline: TimelineService,
}

impl ProtocolSyncService {
    pub fn new(db: SurrealClient, timeline: TimelineService) -> Self {
        Self { db, timeline }
    }

    /// Synchronizes tasks from a markdown file into the database.
    pub async fn sync_from_markdown(&self, path: &Path, project_name: &str, agent_id: &str) -> Result<()> {
        let content = fs::read_to_string(path).await
            .with_context(|| format!("Failed to read markdown file: {:?}", path))?;

        let tasks = self.parse_tasks_from_markdown(&content);

        // Get project ID
        let query = "SELECT * FROM projects WHERE name = $name LIMIT 1";
        let projects: Vec<crate::models::Project> = self.db.query_with(query, ("name", project_name)).await?;
        let project = projects.into_iter().next().context("Project not found")?;
        let project_id_str = project.id.as_ref().map(|t| t.to_string()).unwrap_or_default();

        for mut markdown_task in tasks {
            markdown_task.project_id = project_id_str.clone();
            markdown_task.created_by = agent_id.to_string();

            let ext_id = markdown_task.external_id.clone().unwrap_or_default();
            println!("Syncing markdown task: ID={}, Desc={}", ext_id, markdown_task.description);

            // Check if task already exists by external_id
            let query = "SELECT * FROM tasks WHERE external_id = $ext_id AND project_id = $proj_id LIMIT 1";
            let mut vars = std::collections::HashMap::new();
            vars.insert("ext_id", ext_id.clone());
            vars.insert("proj_id", project_id_str.clone());

            let existing: Vec<Task> = self.db.query_with(query, vars).await?;

            if let Some(existing_task) = existing.into_iter().next() {
                // Update existing task if status changed
                if existing_task.status != markdown_task.status || existing_task.description != markdown_task.description {
                    let task_id = existing_task.id.as_ref().unwrap();
                    let task_id_str = match &task_id.id {
                        surrealdb::sql::Id::String(s) => s.clone(),
                        _ => task_id.to_string(),
                    };

                    self.db.update("tasks", &task_id_str, &markdown_task).await?;

                    self.timeline.emit_task_event(
                        agent_id,
                        EventType::TaskUpdated,
                        &project_id_str,
                        &task_id_str
                    ).await?;
                }
            } else {
                // Create new task
                let created: Task = self.db.create("tasks", &markdown_task).await?;
                let task_id_str = created.id.as_ref().unwrap().to_string();

                self.timeline.emit_task_event(
                    agent_id,
                    EventType::TaskCreated,
                    &project_id_str,
                    &task_id_str
                ).await?;
            }
        }

        Ok(())
    }

    /// Synchronizes task statuses from the database back into the markdown file.
    pub async fn sync_to_markdown(&self, path: &Path, project_name: &str) -> Result<()> {
        let content = fs::read_to_string(path).await
            .with_context(|| format!("Failed to read markdown file: {:?}", path))?;

        // Get project ID
        let query = "SELECT * FROM projects WHERE name = $name LIMIT 1";
        let projects: Vec<crate::models::Project> = self.db.query_with(query, ("name", project_name)).await?;
        let project = projects.into_iter().next().context("Project not found")?;
        let project_id_str = project.id.as_ref().map(|t| t.to_string()).unwrap_or_default();

        // Get all tasks for this project
        let query = "SELECT * FROM tasks WHERE project_id = $proj_id";
        let db_tasks: Vec<Task> = self.db.query_with(query, ("proj_id", &project_id_str)).await?;

        let updated_content = self.update_markdown_with_tasks(&content, &db_tasks);

        if content != updated_content {
            fs::write(path, updated_content).await?;
        }

        Ok(())
    }

    fn parse_tasks_from_markdown(&self, content: &str) -> Vec<Task> {
        let mut tasks = Vec::new();
        let mut in_table = false;
        let mut headers = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('|') {
                let parts: Vec<String> = trimmed
                    .split('|')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if parts.is_empty() { continue; }

                if !in_table {
                    // Possible header
                    if parts.iter().any(|p| p.to_lowercase() == "id") && parts.iter().any(|p| p.to_lowercase() == "status") {
                        headers = parts;
                        in_table = true;
                    }
                    continue;
                }

                if trimmed.contains("---|") || trimmed.contains(":---|") {
                    continue; // Skip separator
                }

                // Table row
                let mut external_id = None;
                let mut status = TaskStatus::Pending;
                let mut description = String::new();

                for (i, part) in parts.iter().enumerate() {
                    if i >= headers.len() { break; }
                    let header = headers[i].to_lowercase();
                    if header == "id" {
                        external_id = Some(part.clone());
                    } else if header == "status" {
                        status = match part.to_lowercase().as_str() {
                            p if p.contains('✅') || p.contains("completed") || p.contains("done") => TaskStatus::Completed,
                            p if p.contains('🔄') || p.contains("running") || p.contains("inprogress") => TaskStatus::Running,
                            p if p.contains('❌') || p.contains("failed") => TaskStatus::Failed,
                            p if p.contains('⏳') || p.contains("pending") || p.contains("todo") => TaskStatus::Pending,
                            _ => TaskStatus::Pending,
                        };
                    } else if header == "task" || header == "description" {
                        description = part.clone();
                    }
                }

                if let Some(id) = external_id {
                    if !id.is_empty() && id != "ID" {
                        let mut t = Task::new("", &description, "", Some(id.clone()));
                        t.status = status;
                        t.external_id = Some(id); // Ensure external_id is set
                        tasks.push(t);
                    }
                }
            } else {
                in_table = false;
            }
        }
        tasks
    }

    fn update_markdown_with_tasks(&self, content: &str, db_tasks: &[Task]) -> String {
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let mut in_table = false;
        let mut headers = Vec::new();

        for i in 0..lines.len() {
            let line = lines[i].trim();
            if line.starts_with('|') {
                let parts: Vec<String> = line
                    .split('|')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if parts.is_empty() { continue; }

                if !in_table {
                    if parts.iter().any(|p| p.to_lowercase() == "id") && parts.iter().any(|p| p.to_lowercase() == "status") {
                        headers = parts;
                        in_table = true;
                    }
                    continue;
                }

                if line.contains("---|") || line.contains(":---|") {
                    continue;
                }

                // Row
                let mut external_id = None;
                let mut status_idx = None;

                for (j, part) in parts.iter().enumerate() {
                    if j >= headers.len() { break; }
                    let header = headers[j].to_lowercase();
                    if header == "id" {
                        external_id = Some(part.clone());
                    } else if header == "status" {
                        status_idx = Some(j);
                    }
                }

                if let (Some(ext_id), Some(s_idx)) = (external_id, status_idx) {
                    if let Some(db_task) = db_tasks.iter().find(|t| t.external_id.as_deref() == Some(&ext_id)) {
                        let status_str = match db_task.status {
                            TaskStatus::Completed => "✅ Completed",
                            TaskStatus::Running => "🔄 Running",
                            TaskStatus::Failed => "❌ Failed",
                            TaskStatus::Cancelled => "🚫 Cancelled",
                            TaskStatus::Pending => "⏳ Pending",
                        };

                        // Update the status in the row
                        let mut new_parts: Vec<String> = line
                            .split('|')
                            .map(|s| s.trim().to_string())
                            .collect();

                        // split('|') on "| ID | Status |" gives ["", " ID ", " Status ", ""]
                        // if headers were ["ID", "Status"], parts were ["ID", "Status"]
                        // indices in parts: 0: ID, 1: Status
                        // indices in new_parts: 0: "", 1: ID, 2: Status, 3: ""

                        if s_idx + 1 < new_parts.len() {
                            new_parts[s_idx + 1] = status_str.to_string();
                            lines[i] = new_parts.join(" | ").trim().to_string();
                            if !lines[i].starts_with('|') {
                                lines[i] = format!("| {} |", lines[i]);
                            }
                        }
                    }
                }
            } else {
                in_table = false;
            }
        }

        lines.join("\n")
    }
}
