use crate::run_state::AgentState;
use gestalt_state::StateDb;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Helper function to retrieve the base path for runs.
///
/// It checks `$GESTALT_HOME` (and joins with `"runs"`) first,
/// then falls back to `~/.gestalt/runs/`.
///
/// Note: `StateDbEventLog` doesn't use this since it stores
/// events in SQLite, but it's kept for API compatibility.
pub fn get_base_dir() -> PathBuf {
    if let Some(gestalt_home) = std::env::var_os("GESTALT_HOME") {
        PathBuf::from(gestalt_home).join("runs")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".gestalt").join("runs")
    } else {
        PathBuf::from(".gestalt").join("runs")
    }
}

/// An event tracked in the timeline of a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    RunStarted {
        run_id: Uuid,
        task: String,
        agents: Vec<String>,
        sha_base: String,
    },
    AgentStateChanged {
        run_id: Uuid,
        agent_id: String,
        from: AgentState,
        to: AgentState,
    },
    CheckpointCommitted {
        commit_hash: String,
    },
    OverlapDetected {
        run_id: Uuid,
        agent_a: String,
        agent_b: String,
        files: Vec<String>,
    },
    MergeConflict {
        run_id: Uuid,
        agent: String,
        path: String,
    },
    MergeComputed {
        target_branch: String,
        success: bool,
    },
    BranchPublished {
        branch: String,
    },
    SymlinkEscape {
        path: String,
    },
    ExcludedFile {
        path: String,
    },
    RunFinished {
        run_id: Uuid,
        summary: String,
    },
}

/// A wrapper to include a schema version in each logged event.
///
/// Used by the legacy `JsonlEventLog` format; retained for
/// deserialization compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionedEvent {
    pub v: usize,
    #[serde(flatten)]
    pub event: Event,
}

/// Trait defining the operations on an append-only timeline log.
pub trait EventLog: Send + Sync {
    fn log(&self, event: Event) -> Result<(), crate::run::RouterError>;
    fn append(&self, event: Event) -> Result<(), crate::run::RouterError>;
    fn read_events(&self, run_id: Uuid) -> Result<Vec<Event>, crate::run::RouterError>;
    fn list_runs(&self) -> Result<Vec<Uuid>, crate::run::RouterError>;
}

/// A `StateDb`-backed implementation of the `EventLog` trait.
///
/// Timeline events are persisted to a SQLite database via the
/// [`StateDb`] backend, providing durable storage, indexed queries,
/// and WAL-mode concurrency.
pub struct StateDbEventLog {
    db: Arc<StateDb>,
    run_id: Uuid,
}

impl StateDbEventLog {
    pub fn new(db: Arc<StateDb>, run_id: Uuid) -> Self {
        Self { db, run_id }
    }
}

impl EventLog for StateDbEventLog {
    fn log(&self, event: Event) -> Result<(), crate::run::RouterError> {
        let agent_id = extract_agent_id(&event);
        let event_type = extract_event_type(&event);

        // Serialize the full event as a JSON payload
        let payload = serde_json::to_value(&event)
            .map_err(|e| crate::run::RouterError::timeline_error(e.to_string()))?;
        let payload_str = serde_json::to_string(&payload)
            .map_err(|e| crate::run::RouterError::timeline_error(e.to_string()))?;

        self.db
            .push_event(
                &self.run_id.to_string(),
                agent_id.as_deref(),
                &event_type,
                &payload_str,
            )
            .map_err(|e| crate::run::RouterError::timeline_error(e.to_string()))?;

        Ok(())
    }

    fn append(&self, event: Event) -> Result<(), crate::run::RouterError> {
        self.log(event)
    }

    fn read_events(&self, run_id: Uuid) -> Result<Vec<Event>, crate::run::RouterError> {
        let events = self
            .db
            .get_timeline(&run_id.to_string(), 1000)
            .map_err(|e| crate::run::RouterError::timeline_error(e.to_string()))?;

        let mut parsed: Vec<Event> = events
            .iter()
            .filter_map(|e| serde_json::from_str(&e.payload).ok())
            .collect();

        // Since get_timeline returns events ordered by seq DESC, reverse them to return in chronological order
        parsed.reverse();

        Ok(parsed)
    }

    fn list_runs(&self) -> Result<Vec<Uuid>, crate::run::RouterError> {
        // Simplified: the timeline table supports queries by run_id
        // but listing all known runs requires a separate query.
        // Return empty for now; callers that need run discovery
        // should query the `runs` table directly.
        Ok(Vec::new())
    }
}

/// Extract the agent ID from an event, if the event type carries one.
fn extract_agent_id(event: &Event) -> Option<String> {
    match event {
        Event::AgentStateChanged { agent_id, .. } => Some(agent_id.clone()),
        _ => None,
    }
}

/// Map an event variant to a short, stable event type string.
fn extract_event_type(event: &Event) -> String {
    match event {
        Event::RunStarted { .. } => "run_started",
        Event::AgentStateChanged { .. } => "state_changed",
        Event::OverlapDetected { .. } => "overlap_detected",
        Event::MergeConflict { .. } => "merge_conflict",
        Event::RunFinished { .. } => "run_finished",
        Event::CheckpointCommitted { .. } => "checkpoint_committed",
        Event::MergeComputed { .. } => "merge_computed",
        Event::BranchPublished { .. } => "branch_published",
        Event::SymlinkEscape { .. } => "symlink_escape",
        Event::ExcludedFile { .. } => "excluded_file",
    }
    .to_string()
}
