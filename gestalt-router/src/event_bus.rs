//! Universal event bus — ingress for events from ANY agent (Hermes, Jules,
//! agent CLI, Gestalt native) with durable persistence + Xavier streaming.
//!
//! Design: `docs/design/xavier-thinking-bus.md` (P0, section 4.1).
//!
//! Flow:
//!   1. Any agent POSTs a [`BusEvent`] to `POST /api/event` (fire-and-forget)
//!   2. [`handle_event`] persists it durably in the SQLite timeline (StateDb)
//!   3. An optional [`XavierEventSink`](crate::xavier_sink::XavierEventSink)
//!      forwards it to Xavier as `kind=execution` in real time
//!
//! The bus is append-only and best-effort: if Xavier is down, events remain in
//! StateDb and can be re-sunk later (cursor-based replay).

use gestalt_state::StateDb;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

/// A structured event pushed onto the universal information bus.
///
/// Carries full traceability metadata: who sent it (`agent`), what happened
/// (`event_type`, `state`), which run/project it belongs to, and free-form
/// `metadata` for LLM/provider/decision context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusEvent {
    /// Originating agent: "hermes" | "jules" | "agent-cli" | "gestalt" | ...
    pub agent: String,
    /// Event kind: "run_started" | "agent_state" | "checkpoint" | "decision" | ...
    pub event_type: String,
    /// Run identifier, when the event belongs to a Gestalt orchestration run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Project name (e.g. "gestalt", "xavier", "nido").
    #[serde(default)]
    pub project: Option<String>,
    /// Agent state: Pending | Running | Success | Timeout | Crashed | NoChanges.
    #[serde(default)]
    pub state: Option<String>,
    /// Human-readable one-line summary of what happened.
    pub summary: String,
    /// Free-form traceability metadata: {"llm": "...", "provider": "...",
    /// "decision": "...", "requested_by": "...", "tool_calls": N, ...}.
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// RFC3339 timestamp of the event.
    #[serde(default)]
    pub ts: String,
}

impl BusEvent {
    /// Build a `BusEvent` filling `ts` with the current UTC RFC3339 time.
    pub fn new(
        agent: impl Into<String>,
        event_type: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            agent: agent.into(),
            event_type: event_type.into(),
            run_id: None,
            project: None,
            state: None,
            summary: summary.into(),
            metadata: serde_json::Value::Null,
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Attach a run id.
    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    /// Attach a project name.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Attach an agent state.
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Attach traceability metadata (llm, provider, decision, requested_by, ...).
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// The timeline run_id used for persistence: the event's own run_id when
    /// present, otherwise a synthetic "bus" bucket so events without a run
    /// still land in a queryable timeline stream.
    pub fn timeline_run_id(&self) -> String {
        self.run_id.clone().unwrap_or_else(|| "bus".to_string())
    }
}

/// Persist a bus event durably in the StateDb timeline.
///
/// This is step 1 of the P0 flow — durable storage that survives a Xavier
/// outage. Returns the assigned sequence number.
pub fn persist_event(db: &StateDb, ev: &BusEvent) -> Result<i64, String> {
    let payload = serde_json::to_string(ev).map_err(|e| e.to_string())?;
    db.push_event(
        &ev.timeline_run_id(),
        Some(&ev.agent),
        &ev.event_type,
        &payload,
    )
    .map(|t| t.seq.unwrap_or(0))
    .map_err(|e| e.to_string())
}

/// Handle a bus event end-to-end: durable persistence (always) + broadcast +
/// optional Xavier streaming (fire-and-forget).
///
/// - `db`: durable SQLite timeline (required).
/// - `sink`: optional real-time forwarder to Xavier (`kind=execution`).
///   Errors are logged as warnings and never block the caller.
pub async fn handle_event(
    db: &Arc<StateDb>,
    ev: &BusEvent,
    sink: Option<&crate::xavier_sink::XavierEventSink>,
) -> Result<i64, String> {
    let seq = persist_event(db, ev)?;

    if let Some(sink) = sink {
        if let Err(e) = sink.sink(ev).await {
            warn!(
                agent = %ev.agent,
                event_type = %ev.event_type,
                "bus → Xavier sink failed (event kept in StateDb): {}",
                e
            );
        }
    }

    Ok(seq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_event_roundtrip_serialization() {
        let ev = BusEvent::new("hermes", "run_finished", "verify-pipeline 27/27 PASS")
            .with_run_id("run-42")
            .with_project("xavier")
            .with_state("Success")
            .with_metadata(serde_json::json!({
                "llm": "deepseek-v4-flash",
                "provider": "opencode",
                "requested_by": "bela",
                "tool_calls": 42,
            }));

        let json = serde_json::to_string(&ev).unwrap();
        let back: BusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
        assert!(back.ts.contains("T"));
    }

    #[test]
    fn bus_event_defaults_are_safe() {
        let ev = BusEvent::new("gestalt", "checkpoint", "checkpoint committed");
        assert_eq!(ev.run_id, None);
        assert_eq!(ev.project, None);
        assert_eq!(ev.state, None);
        assert!(ev.metadata.is_null());
        assert_eq!(ev.timeline_run_id(), "bus");
        assert!(!ev.ts.is_empty());
    }

    #[test]
    fn bus_event_timeline_run_id_uses_own_run() {
        let ev = BusEvent::new("jules", "agent_state", "running")
            .with_run_id("01ABCD")
            .with_state("Running");
        assert_eq!(ev.timeline_run_id(), "01ABCD");
    }
}
