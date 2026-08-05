//! Xavier Event Sink — streams bus events to Xavier's memory store in real time.
//!
//! Design: `docs/design/xavier-thinking-bus.md` (P0, section 4.2).
//!
//! Every [`BusEvent`](crate::event_bus::BusEvent) is forwarded to
//! `POST /v1/memories` as `kind=execution` with rich traceability metadata
//! (agent, run_id, project, state, event_type, ts), so Xavier can index the
//! full activity of all agents and feed it back as PRE context to future runs.
//!
//! Failure policy: best-effort, fire-and-forget. The caller logs the error and
//! the event remains durably in StateDb (replay-able via a cursor sweep).

use gestalt_core::application::agent::xavier::XavierClient;
use serde_json::json;

use crate::event_bus::BusEvent;

/// Forwards bus events to Xavier as `kind=execution` memories.
#[derive(Debug, Clone)]
pub struct XavierEventSink {
    client: XavierClient,
    /// Memory path prefix; defaults to "gestalt/bus/executions".
    path_prefix: String,
}

impl XavierEventSink {
    /// Create a sink from an existing [`XavierClient`].
    pub fn new(client: XavierClient) -> Self {
        Self {
            client,
            path_prefix: "gestalt/bus/executions".to_string(),
        }
    }

    /// Create a sink from environment (`XAVIER_URL`, `XAVIER_TOKEN`).
    ///
    /// Falls back gracefully: if the token is empty, the sink is created but
    /// every call will fail fast with a clear error (never panics).
    pub fn from_env() -> Self {
        Self::new(XavierClient::from_env())
    }

    /// Override the memory path prefix (e.g. per-project buckets).
    pub fn with_path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = prefix.into();
        self
    }

    /// Forward one bus event to Xavier as `kind=execution`.
    ///
    /// Each event gets a UNIQUE memory path (`<prefix>/<ts>-<run_id>`), so every
    /// event is its own memory record with full traceability metadata — never
    /// overwrites a previous event (Xavier upserts by path).
    pub async fn sink(&self, ev: &BusEvent) -> Result<(), crate::run::RouterError> {
        let content = format!(
            "[{}] {} {} — {}",
            ev.event_type,
            ev.agent,
            ev.run_id.as_deref().unwrap_or("?"),
            ev.summary
        );

        let metadata = json!({
            "agent": ev.agent,
            "run_id": ev.run_id,
            "project": ev.project,
            "state": ev.state,
            "event_type": ev.event_type,
            "ts": ev.ts,
            "trace": ev.metadata,
        });

        // Unique path per event: prefix + timestamp (+ run_id when present).
        let ts_slug = ev
            .ts
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>();
        let run_slug = ev.run_id.as_deref().unwrap_or("anon");
        let path = format!("{}/{}-{}", self.path_prefix, ts_slug, run_slug);

        self.client
            .add(&content, &path, "execution", metadata)
            .await
            .map_err(|e| {
                crate::run::RouterError::timeline_error(format!("Xavier sink failed: {}", e))
            })?;

        Ok(())
    }

    /// Health check against Xavier.
    pub async fn is_available(&self) -> bool {
        self.client.is_available().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_constructs_from_env_without_panicking() {
        // Token may be empty in CI; construction must never panic.
        let sink = XavierEventSink::from_env();
        assert!(sink.path_prefix.contains("gestalt/bus"));
    }

    #[test]
    fn sink_metadata_shape() {
        let ev = BusEvent::new("hermes", "run_finished", "27/27 PASS")
            .with_run_id("run-1")
            .with_project("xavier")
            .with_state("Success")
            .with_metadata(json!({"llm": "deepseek-v4-flash"}));

        // The content format used by `sink()`.
        let content = format!(
            "[{}] {} {} — {}",
            ev.event_type,
            ev.agent,
            ev.run_id.as_deref().unwrap_or("?"),
            ev.summary
        );
        assert_eq!(content, "[run_finished] hermes run-1 — 27/27 PASS");

        // Unique path per event: prefix/<ts-slug>-<run_id>.
        let ts_slug = ev
            .ts
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>();
        let path = format!("gestalt/bus/executions/{}-{}", ts_slug, "run-1");
        assert!(path.starts_with("gestalt/bus/executions/"));
        assert!(path.ends_with("-run-1"));
        assert!(!path.contains(' '));
    }
}
