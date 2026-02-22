//! Timeline Service - Core of the system
//!
//! Manages the universal timeline where all events are recorded.

use anyhow::Result;
use chrono::{Duration, Utc};
use tracing::debug;

use crate::db::SurrealClient;
use crate::models::{EventType, TimelineEvent};

/// Service for managing the universal timeline.
#[derive(Clone)]
pub struct TimelineService {
    db: SurrealClient,
}

impl TimelineService {
    /// Create a new TimelineService.
    pub fn new(db: SurrealClient) -> Self {
        Self { db }
    }

    /// Record an event in the timeline.
    ///
    /// This is the core operation of the system. Every action gets a timestamp.
    pub async fn record_event(&self, event: TimelineEvent) -> Result<TimelineEvent> {
        debug!("Recording timeline event: {:?}", event.event_type);
        self.db.create("timeline_events", &event).await
    }

    /// Create and record a new event.
    pub async fn emit(&self, agent_id: &str, event_type: EventType) -> Result<TimelineEvent> {
        let event = TimelineEvent::new(agent_id, event_type);
        self.record_event(event).await
    }

    /// Create and record a project-related event.
    pub async fn emit_project_event(
        &self,
        agent_id: &str,
        event_type: EventType,
        project_id: &str,
    ) -> Result<TimelineEvent> {
        let event = TimelineEvent::new(agent_id, event_type).with_project(project_id);
        self.record_event(event).await
    }

    /// Create and record a task-related event.
    pub async fn emit_task_event(
        &self,
        agent_id: &str,
        event_type: EventType,
        project_id: &str,
        task_id: &str,
    ) -> Result<TimelineEvent> {
        let event = TimelineEvent::new(agent_id, event_type)
            .with_project(project_id)
            .with_task(task_id);
        self.record_event(event).await
    }

    /// Get timeline events, optionally filtered by duration.
    ///
    /// # Arguments
    /// * `since` - Duration string like "1h", "30m", "2d"
    pub async fn get_timeline(&self, since: Option<&str>) -> Result<Vec<TimelineEvent>> {
        let since_time = match since {
            Some(s) => self.parse_duration(s)?,
            None => Utc::now() - Duration::hours(24), // Default to last 24 hours
        };

        let query = r#"
            SELECT * FROM timeline_events
            WHERE timestamp >= $since
            ORDER BY timestamp DESC
            LIMIT 100
        "#;

        let events: Vec<TimelineEvent> = self.db.query_with(query, ("since", since_time)).await?;

        Ok(events)
    }

    /// Get timeline events for a specific project.
    pub async fn get_project_timeline(&self, project_id: &str) -> Result<Vec<TimelineEvent>> {
        let query = r#"
            SELECT * FROM timeline_events
            WHERE project_id = $project_id
            ORDER BY timestamp DESC
            LIMIT 100
        "#;

        let events: Vec<TimelineEvent> = self
            .db
            .query_with(query, ("project_id", project_id))
            .await?;

        Ok(events)
    }

    /// Get events strictly after a specific timestamp.
    pub async fn get_events_since(
        &self,
        since: chrono::DateTime<Utc>,
    ) -> Result<Vec<TimelineEvent>> {
        let query = r#"
            SELECT * FROM timeline_events
            WHERE timestamp > $since
            ORDER BY timestamp ASC
        "#;

        let events: Vec<TimelineEvent> = self.db.query_with(query, ("since", since)).await?;

        Ok(events)
    }

    /// Parse a duration string like "1h", "30m", "2d" into a DateTime.
    fn parse_duration(&self, s: &str) -> Result<chrono::DateTime<Utc>> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(Utc::now() - Duration::hours(24));
        }

        let (num_str, unit) = s.split_at(s.len().saturating_sub(1));
        let num: i64 = num_str.parse().unwrap_or(1);

        let duration = match unit {
            "m" => Duration::minutes(num),
            "h" => Duration::hours(num),
            "d" => Duration::days(num),
            "w" => Duration::weeks(num),
            _ => Duration::hours(num), // Default to hours
        };

        Ok(Utc::now() - duration)
    }
}
