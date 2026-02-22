//! Project Service

use anyhow::{Context, Result};
use surrealdb::sql::Thing;
use tracing::debug;

use crate::db::SurrealClient;
use crate::models::{
    EventType, FlexibleTimestamp, Project, ProjectStatus, ProjectStatusInfo, Task,
};
use crate::services::TimelineService;

/// Helper to convert Option<Thing> to String
fn thing_to_string(thing: &Option<Thing>) -> String {
    thing.as_ref().map(|t| t.to_string()).unwrap_or_default()
}

/// Service for managing projects.
#[derive(Clone)]
pub struct ProjectService {
    db: SurrealClient,
    timeline: TimelineService,
}

impl ProjectService {
    /// Create a new ProjectService.
    pub fn new(db: SurrealClient, timeline: TimelineService) -> Self {
        Self { db, timeline }
    }

    /// Create a new project.
    pub async fn create_project(&self, name: &str, agent_id: &str) -> Result<Project> {
        debug!("Creating project: {}", name);

        let project = Project::new(name, agent_id);

        // Store in database
        let created: Project = self.db.create("projects", &project).await?;

        // Record timeline event
        let project_id_str = thing_to_string(&created.id);
        self.timeline
            .emit_project_event(agent_id, EventType::ProjectCreated, &project_id_str)
            .await?;

        Ok(created)
    }

    /// List all projects.
    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        self.db.select_all("projects").await
    }

    /// Get a project by name.
    pub async fn get_by_name(&self, name: &str) -> Result<Option<Project>> {
        let query = "SELECT * FROM projects WHERE name = $name LIMIT 1";
        let projects: Vec<Project> = self.db.query_with(query, ("name", name)).await?;
        Ok(projects.into_iter().next())
    }

    /// Get a project by ID.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Project>> {
        self.db.select_by_id("projects", id).await
    }

    /// Get project status with task counts.
    pub async fn get_status(&self, project_name: &str) -> Result<ProjectStatusInfo> {
        let project = self
            .get_by_name(project_name)
            .await?
            .context("Project not found")?;

        // Get tasks for this project
        let project_id_str = thing_to_string(&project.id);
        let query = "SELECT * FROM tasks WHERE project_id = $project_id";
        let tasks: Vec<Task> = self
            .db
            .query_with(query, ("project_id", &project_id_str))
            .await?;

        let total_tasks = tasks.len();
        let completed_tasks = tasks
            .iter()
            .filter(|t| t.status == crate::models::TaskStatus::Completed)
            .count();

        let progress_percent = if total_tasks > 0 {
            ((completed_tasks as f64 / total_tasks as f64) * 100.0) as u8
        } else {
            0
        };

        Ok(ProjectStatusInfo {
            id: project_id_str,
            name: project.name,
            status: project.status,
            total_tasks,
            completed_tasks,
            progress_percent,
        })
    }

    /// Update project status.
    pub async fn update_status(
        &self,
        project_id: &str,
        status: ProjectStatus,
        agent_id: &str,
    ) -> Result<Project> {
        let mut project = self
            .get_by_id(project_id)
            .await?
            .context("Project not found")?;

        project.status = status;
        project.updated_at = FlexibleTimestamp::now();

        let updated = self.db.update("projects", project_id, &project).await?;

        self.timeline
            .emit_project_event(agent_id, EventType::ProjectUpdated, project_id)
            .await?;

        Ok(updated)
    }

    /// Delete a project and its tasks.
    pub async fn delete_project(&self, project_id: &str, agent_id: &str) -> Result<()> {
        debug!("Deleting project: {}", project_id);

        // Delete tasks first
        let query = "DELETE FROM tasks WHERE project_id = $project_id";
        self.db
            .query_with::<serde_json::Value>(query, ("project_id", project_id))
            .await?;

        // Delete project
        self.db.delete("projects", project_id).await?;

        // Emit event
        self.timeline
            .emit_project_event(agent_id, EventType::ProjectDeleted, project_id)
            .await?;

        Ok(())
    }
}
