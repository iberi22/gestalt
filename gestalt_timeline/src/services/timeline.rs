//! Timeline Service - Core of the system
//!
//! Manages the universal timeline where all events are recorded.

use anyhow::Result;
use chrono::{Duration, Utc};
use reqwest::Client;
use std::time::Duration as StdDuration;
use tracing::{debug, info, warn};

use crate::db::SurrealClient;
use crate::models::{EventType, TimelineEvent};

/// Service for managing the universal timeline.
#[derive(Clone)]
pub struct TimelineService {
    db: SurrealClient,
    cortex_sync: Option<CortexSync>,
    sync_enabled: bool,
}

/// Cortex sync client for timeline events
#[derive(Clone)]
struct CortexSync {
    client: Client,
    url: String,
    token: String,
}

impl CortexSync {
    fn new(url: String, token: String) -> Self {
        let client = Client::builder()
            .timeout(StdDuration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, url, token }
    }

    /// Sync a timeline event to Cortex as a memory
    async fn sync_event(&self, event: &TimelineEvent) -> Result<()> {
        let path = format!(
            "timeline/{}/{}/{}",
            event.agent_id,
            event.event_type,
            event.timestamp.unix_timestamp()
        );

        let content = format!(
            "[{}] {}: {:?}",
            event.timestamp.to_rfc3339(),
            event.agent_id,
            event.event_type
        );

        let metadata = serde_json::json!({
            "agent_id": event.agent_id.clone(),
            "event_type": event.event_type.to_string(),
            "project_id": event.project_id,
            "task_id": event.task_id,
            "payload": event.payload,
            "timestamp": event.timestamp.to_rfc3339(),
        });

        let memory = serde_json::json!({
            "path": path,
            "content": content,
            "kind": "timeline_event",
            "metadata": metadata,
        });

        let response = self
            .client
            .post(format!("{}/memory/add", self.url))
            .header("X-Cortex-Token", &self.token)
            .json(&memory)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Cortex sync failed: {} - {}",
                status,
                body
            ));
        }

        Ok(())
    }

    /// Check if Cortex is available
    async fn is_available(&self) -> bool {
        let response = self
            .client
            .get(format!("{}/health", self.url))
            .header("X-Cortex-Token", &self.token)
            .send()
            .await;

        match response {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

impl TimelineService {
    /// Create a new TimelineService.
    pub fn new(db: SurrealClient) -> Self {
        let cortex_sync = Self::create_cortex_sync();
        Self {
            db,
            cortex_sync,
            sync_enabled: cortex_sync.is_some(),
        }
    }

    /// Create Cortex sync client from env vars or config
    fn create_cortex_sync() -> Option<CortexSync> {
        let url = std::env::var("CORTEX_URL").ok().unwrap_or_else(|| "http://localhost:8003".to_string());
        let token = std::env::var("CORTEX_TOKEN").ok().unwrap_or_else(|| "dev-token".to_string());

        if url.is_empty() {
            return None;
        }

        Some(CortexSync::new(url, token))
    }

    /// Create with explicit Cortex settings
    pub fn with_cortex(db: SurrealClient, cortex_url: &str, cortex_token: &str) -> Self {
        let cortex_sync = Some(CortexSync::new(
            cortex_url.to_string(),
            cortex_token.to_string(),
        ));
        Self {
            db,
            cortex_sync,
            sync_enabled: true,
        }
    }

    /// Enable or disable Cortex sync
    pub fn set_sync_enabled(&mut self, enabled: bool) {
        self.sync_enabled = enabled;
    }

    /// Check if Cortex sync is available and enabled
    async fn is_cortex_sync_available(&self) -> bool {
        if !self.sync_enabled {
            return false;
        }
        if let Some(ref sync) = self.cortex_sync {
            sync.is_available().await
        } else {
            false
        }
    }

    /// Sync an event to Cortex (if enabled and available)
    async fn sync_to_cortex(&self, event: &TimelineEvent) {
        if !self.sync_enabled {
            return;
        }

        if let Some(ref sync) = self.cortex_sync {
            match sync.sync_event(event).await {
                Ok(_) => {
                    debug!("Synced event to Cortex: {:?}", event.event_type);
                }
                Err(e) => {
                    warn!("Failed to sync event to Cortex: {}", e);
                }
            }
        }
    }

    /// Record an event in the timeline.
    ///
    /// This is the core operation of the system. Every action gets a timestamp.
    /// If Cortex sync is enabled, also syncs to Cortex.
    pub async fn record_event(&self, event: TimelineEvent) -> Result<TimelineEvent> {
        debug!("Recording timeline event: {:?}", event.event_type);

        // Record to SurrealDB
        let recorded = self.db.create("timeline_events", &event).await?;

        // Optionally sync to Cortex
        self.sync_to_cortex(&recorded).await;

        Ok(recorded)
    }

    /// Record an event without syncing to Cortex (for bulk operations)
    pub async fn record_event_local(&self, event: TimelineEvent) -> Result<TimelineEvent> {
        debug!("Recording timeline event (local only): {:?}", event.event_type);
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
