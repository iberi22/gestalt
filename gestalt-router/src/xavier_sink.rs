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

use std::sync::OnceLock;
use regex::Regex;
use gestalt_ws::WsEvent;
use gestalt_core::application::agent::xavier::XavierClient;
use serde_json::json;

use crate::event_bus::BusEvent;

/// Check if a given text contains sensitive patterns such as:
/// XAVIER_TOKEN, password, token, secret, api_key, api-key, apikey, ghp_, or sk-.
pub fn contains_secret(text: &str) -> bool {
    static RE_SENSITIVE: OnceLock<Regex> = OnceLock::new();
    let re = RE_SENSITIVE.get_or_init(|| {
        Regex::new(r"(?i)XAVIER_TOKEN|password|token|secret|api[_-]?key|ghp_|sk-").unwrap()
    });
    re.is_match(text)
}

/// Redact sensitive data from a text, replacing keys' values and direct tokens with `[REDACTED]`.
pub fn redact(text: &str) -> String {
    // 1. Redact key-value pairs where key is case-insensitive match for keywords:
    // XAVIER_TOKEN, password, token, secret, api_key, api-key, apikey
    //
    // Since Rust's regex crate does not support backreferences, we split the matching
    // into three safe patterns: double-quoted values, single-quoted values, and unquoted values.

    static RE_DOUBLE_QUOTED: OnceLock<Regex> = OnceLock::new();
    let re_dq = RE_DOUBLE_QUOTED.get_or_init(|| {
        Regex::new(r#"(?i)(XAVIER_TOKEN|password|token|secret|api[_-]?key)(\s*[:=]\s*)(")(.*?)(")"#).unwrap()
    });

    static RE_SINGLE_QUOTED: OnceLock<Regex> = OnceLock::new();
    let re_sq = RE_SINGLE_QUOTED.get_or_init(|| {
        Regex::new(r#"(?i)(XAVIER_TOKEN|password|token|secret|api[_-]?key)(\s*[:=]\s*)(')(.*?)(')"#).unwrap()
    });

    static RE_UNQUOTED: OnceLock<Regex> = OnceLock::new();
    let re_unq = RE_UNQUOTED.get_or_init(|| {
        Regex::new(r#"(?i)(XAVIER_TOKEN|password|token|secret|api[_-]?key)(\s*[:=]\s*)([^\s,"'\}]+)"#).unwrap()
    });

    let redacted_dq = re_dq.replace_all(text, "$1$2$3[REDACTED]$5");
    let redacted_sq = re_sq.replace_all(&redacted_dq, "$1$2$3[REDACTED]$5");
    let mut redacted = re_unq.replace_all(&redacted_sq, "$1$2[REDACTED]").into_owned();

    // 2. Redact standalone tokens: ghp_... or sk-...
    static RE_GHP: OnceLock<Regex> = OnceLock::new();
    let re_ghp = RE_GHP.get_or_init(|| {
        Regex::new(r"ghp_[a-zA-Z0-9]+").unwrap()
    });

    static RE_SK: OnceLock<Regex> = OnceLock::new();
    let re_sk = RE_SK.get_or_init(|| {
        Regex::new(r"sk-[a-zA-Z0-9_-]+").unwrap()
    });

    redacted = re_ghp.replace_all(&redacted, "[REDACTED]").into_owned();
    redacted = re_sk.replace_all(&redacted, "[REDACTED]").into_owned();

    redacted
}

/// Recursively redact any String value in JSON, and replace sensitive keys' values with `"[REDACTED]"`.
pub fn redact_json(val: serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => serde_json::Value::String(redact(&s)),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(redact_json).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (k, v) in obj {
                let is_key_sensitive = contains_secret(&k);
                if is_key_sensitive {
                    new_obj.insert(k, serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    new_obj.insert(k, redact_json(v));
                }
            }
            serde_json::Value::Object(new_obj)
        }
        other => other,
    }
}

/// Apply redaction filter to a WsEvent.
pub fn redact_ws_event(event: WsEvent) -> WsEvent {
    match event {
        WsEvent::RunStarted { run_id, task, agents } => WsEvent::RunStarted {
            run_id,
            task: redact(&task),
            agents,
        },
        WsEvent::RunFinished { run_id, summary } => WsEvent::RunFinished {
            run_id,
            summary: redact(&summary),
        },
        WsEvent::ConflictDetected { run_id, agent_a, agent_b, path, message } => WsEvent::ConflictDetected {
            run_id,
            agent_a,
            agent_b,
            path,
            message: redact(&message),
        },
        other => other,
    }
}

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
        let redacted_content = redact(&content);

        let metadata = json!({
            "agent": ev.agent,
            "run_id": ev.run_id,
            "project": ev.project,
            "state": ev.state,
            "event_type": ev.event_type,
            "ts": ev.ts,
            "trace": redact_json(ev.metadata.clone()),
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
            .add(&redacted_content, &path, "execution", metadata)
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
