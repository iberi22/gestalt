//! Watch Service - Real-time event observation
//!
//! Provides a persistent process that streams timeline events in real-time.

use anyhow::Result;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::db::SurrealClient;
use crate::models::{EventType, TimelineEvent};
use crate::services::TimelineService;

/// Message types for the watch broadcast channel
#[derive(Debug, Clone)]
pub enum WatchMessage {
    /// A new timeline event occurred
    Event(Box<TimelineEvent>),
    /// A broadcast message from another agent
    Broadcast { agent_id: String, message: String },
    /// System shutdown signal
    Shutdown,
}

/// Service for real-time event observation.
///
/// This service runs as a persistent process that doesn't terminate
/// while in execution, allowing agents to observe events in real-time.
pub struct WatchService {
    db: SurrealClient,
    timeline: TimelineService,
    tx: broadcast::Sender<WatchMessage>,
    running: Arc<AtomicBool>,
}

impl WatchService {
    /// Create a new WatchService.
    pub fn new(db: SurrealClient, timeline: TimelineService) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            db,
            timeline,
            tx,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a receiver for watch messages.
    pub fn subscribe(&self) -> broadcast::Receiver<WatchMessage> {
        self.tx.subscribe()
    }

    /// Check if the watch service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Start watching timeline events.
    ///
    /// This is the persistent process that runs until cancelled.
    /// It polls for new events and broadcasts them to all subscribers.
    pub async fn start_watching(
        &self,
        agent_id: &str,
        project_filter: Option<&str>,
        event_filter: Option<Vec<String>>,
    ) -> Result<()> {
        info!("🔭 Starting watch mode for agent: {}", agent_id);
        self.running.store(true, Ordering::SeqCst);

        // Record agent connection
        self.timeline
            .emit(agent_id, EventType::AgentConnected)
            .await?;

        let mut last_check = Utc::now();
        let poll_interval = tokio::time::Duration::from_millis(500);

        // Setup graceful shutdown
        let running = self.running.clone();

        println!("🔭 Watch mode active. Press Ctrl+C to stop.");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            // Query for new events since last check
            // Note: In SurrealDB, string comparison works for ISO8601 timestamps
            let query = match project_filter {
                Some(pid) => format!(
                    "SELECT * FROM timeline_events WHERE timestamp > $since AND project_id = '{}' ORDER BY timestamp ASC",
                    pid
                ),
                None => "SELECT * FROM timeline_events WHERE timestamp > $since ORDER BY timestamp ASC".to_string(),
            };

            let events: Vec<TimelineEvent> = self
                .db
                .query_with(&query, ("since", last_check))
                .await
                .unwrap_or_default();

            for event in events {
                // Use the inner DateTime<Utc> for comparison and updating last_check
                let ts_utc = event.timestamp.0;

                // Apply event type filter if specified
                if let Some(ref filters) = event_filter {
                    let event_str = event.event_type.to_string();
                    if !filters.iter().any(|f| event_str.contains(f)) {
                        continue;
                    }
                }

                // Print event to console
                println!(
                    "{} │ {:15} │ {:20} │ {}",
                    ts_utc.format("%H:%M:%S"),
                    event.agent_id,
                    event.event_type,
                    event.id.as_ref().map(|x| x.to_string()).unwrap_or_else(|| "none".to_string())
                );

                // Update last_check to strictly follow the latest event seen
                if ts_utc > last_check {
                    last_check = ts_utc;
                }

                // Broadcast to subscribers
                let _ = self.tx.send(WatchMessage::Event(Box::new(event.clone())));
            }

            tokio::time::sleep(poll_interval).await;
        }

        // Record agent disconnection
        self.timeline
            .emit(agent_id, EventType::AgentDisconnected)
            .await?;

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔭 Watch mode stopped.");

        Ok(())
    }

    /// Stop the watch service gracefully.
    pub fn stop(&self) {
        debug!("Stopping watch service");
        self.running.store(false, Ordering::SeqCst);
        let _ = self.tx.send(WatchMessage::Shutdown);
    }

    /// Broadcast a message to all connected agents.
    pub async fn broadcast_message(
        &self,
        agent_id: &str,
        message: &str,
        project_id: Option<&str>,
    ) -> Result<TimelineEvent> {
        info!("📢 Broadcasting message from {}: {}", agent_id, message);

        let mut event = TimelineEvent::new(agent_id, EventType::Custom("broadcast".to_string()))
            .with_payload(serde_json::json!({ "message": message }));

        if let Some(pid) = project_id {
            event = event.with_project(pid);
        }

        let recorded = self.timeline.record_event(event).await?;

        // Send to local subscribers
        let _ = self.tx.send(WatchMessage::Broadcast {
            agent_id: agent_id.to_string(),
            message: message.to_string(),
        });

        Ok(recorded)
    }
}

impl Clone for WatchService {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            timeline: self.timeline.clone(),
            tx: self.tx.clone(),
            running: self.running.clone(),
        }
    }
}
