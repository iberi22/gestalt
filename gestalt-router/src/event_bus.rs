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

/// How many seconds back we consider an event a duplicate (same stable hash).
pub const DEDUP_WINDOW_SECS: i64 = 300;

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

    /// Override the RFC3339 timestamp (mainly for tests).
    pub fn with_ts(mut self, ts: impl Into<String>) -> Self {
        self.ts = ts.into();
        self
    }

    /// The timeline run_id used for persistence: the event's own run_id when
    /// present, otherwise a synthetic "bus" bucket so events without a run
    /// still land in a queryable timeline stream.
    pub fn timeline_run_id(&self) -> String {
        self.run_id.clone().unwrap_or_else(|| "bus".to_string())
    }

    /// Stable content hash used for dedup: SHA-256 over the SEMANTIC identity
    /// of the event (agent + type + run + project + state + summary).
    ///
    /// NOTE: `ts` is intentionally EXCLUDED — emitters generate a fresh
    /// timestamp on every push (e.g. event.py), so including it would make
    /// every push look unique and dedup would never fire. Two pushes of the
    /// same logical event (retries, replay, cron tick re-sent) share the same
    /// hash and are skipped within the dedup window.
    pub fn dedup_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.agent.as_bytes());
        hasher.update([0u8]);
        hasher.update(self.event_type.as_bytes());
        hasher.update([0u8]);
        hasher.update(self.run_id.as_deref().unwrap_or("").as_bytes());
        hasher.update([0u8]);
        hasher.update(self.project.as_deref().unwrap_or("").as_bytes());
        hasher.update([0u8]);
        hasher.update(self.state.as_deref().unwrap_or("").as_bytes());
        hasher.update([0u8]);
        hasher.update(self.summary.as_bytes());
        format!("{:x}", hasher.finalize())
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

/// Check whether an identical event (same dedup hash) was persisted within the
/// dedup window. Retries, replays and double-pushes are skipped — this keeps
/// both StateDb and Xavier free of repetitive duplicates.
///
/// The window is measured against the SERVER-side `created_at` of existing
/// timeline rows (not the emitter-provided `ts`), so emitters with skewed
/// clocks still deduplicate correctly.
pub fn is_duplicate(db: &StateDb, ev: &BusEvent) -> Result<bool, String> {
    let hash = ev.dedup_hash();
    let recent = db
        .recent_timeline(500)
        .map_err(|e| format!("Failed to read timeline for dedup: {}", e))?;

    for existing in recent {
        let parsed: BusEvent = match serde_json::from_str(&existing.payload) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if parsed.dedup_hash() == hash {
            // Same event identity; only a duplicate if the existing row was
            // created within the dedup window (server clock).
            let now = chrono::Utc::now();
            let age_secs = (now - existing.created_at).num_seconds();
            if age_secs <= DEDUP_WINDOW_SECS {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Handle a bus event end-to-end: dedup check (skip repeats) → durable
/// persistence (always) → optional Xavier streaming (fire-and-forget).
///
/// - `db`: durable SQLite timeline (required).
/// - `sink`: optional real-time forwarder to Xavier (`kind=execution`).
///   Errors are logged as warnings and never block the caller.
///
/// Returns `Ok(Some(seq))` when persisted, `Ok(None)` when skipped as a
/// duplicate within the dedup window.
pub async fn handle_event(
    db: &Arc<StateDb>,
    ev: &BusEvent,
    sink: Option<&crate::xavier_sink::XavierEventSink>,
) -> Result<Option<i64>, String> {
    // Dedup: skip identical events within the window (retries/replays).
    if is_duplicate(db, ev)? {
        warn!(
            agent = %ev.agent,
            event_type = %ev.event_type,
            run_id = ?ev.run_id,
            "bus event SKIPPED (duplicate within window)"
        );
        return Ok(None);
    }

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

    Ok(Some(seq))
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

    #[test]
    fn dedup_hash_is_stable_and_distinct() {
        let a = BusEvent::new("hermes", "run_finished", "27/27 PASS")
            .with_run_id("r1")
            .with_ts("2026-08-05T10:00:00Z");
        let b = BusEvent::new("hermes", "run_finished", "27/27 PASS")
            .with_run_id("r1")
            .with_ts("2026-08-05T10:00:01Z");
        let c = BusEvent::new("hermes", "run_finished", "27/27 PASS")
            .with_run_id("r1")
            .with_ts("2026-08-05T10:01:00Z")
            .with_metadata(serde_json::json!({"llm": "deepseek"}));

        // ts is NOT part of the semantic identity — same event, different ts.
        assert_eq!(
            a.dedup_hash(),
            b.dedup_hash(),
            "same event with different ts shares hash"
        );
        assert_eq!(
            a.dedup_hash(),
            c.dedup_hash(),
            "metadata is not part of the semantic identity either"
        );
        assert_eq!(a.dedup_hash().len(), 64, "sha256 hex");

        // Different summary → different hash.
        let d = BusEvent::new("hermes", "run_finished", "28/28 PASS")
            .with_run_id("r1")
            .with_ts("2026-08-05T10:00:00Z");
        assert_ne!(
            a.dedup_hash(),
            d.dedup_hash(),
            "different summary → different hash"
        );
    }

    #[tokio::test]
    async fn handle_event_dedups_identical_pushes() {
        let db = Arc::new(
            StateDb::open(
                std::env::temp_dir()
                    .join(format!("gestalt-dedup-test-{}.db", uuid::Uuid::new_v4())),
            )
            .unwrap(),
        );

        let ev = BusEvent::new("hermes", "run_finished", "dedup test")
            .with_run_id("run-dedup")
            .with_ts(chrono::Utc::now().to_rfc3339());

        // First push persists.
        let first = handle_event(&db, &ev, None).await.unwrap();
        assert!(first.is_some(), "first push persists");

        // Identical push within the window is skipped.
        let second = handle_event(&db, &ev, None).await.unwrap();
        assert!(second.is_none(), "duplicate push is deduplicated");
    }
}
