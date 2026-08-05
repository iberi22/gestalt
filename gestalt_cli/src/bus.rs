//! Gestalt Universal Event Bus — HTTP ingress + serve.
//!
//! Design: `docs/design/xavier-thinking-bus.md` (P0, section 4.1 + 4.2).
//!
//! Exposes:
//! - `POST /api/event`  — any agent pushes a [`BusEvent`] (fire-and-forget)
//! - `GET  /api/events` — tail of recent bus events from StateDb (dashboard)
//! - `GET  /healthz`    — liveness probe for supervisors
//!
//! Events are persisted durably in StateDb and streamed to Xavier in real time
//! as `kind=execution` via the [`XavierEventSink`], giving full traceability
//! (agent, llm, provider, decision, requested_by) across all agents.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use gestalt_router::{
    event_bus::{handle_event, BusEvent},
    xavier_sink::XavierEventSink,
};
use gestalt_state::StateDb;
use std::sync::Arc;
use tracing::{info, warn};

/// Shared application state for the bus HTTP server.
#[derive(Clone)]
pub struct BusState {
    pub db: Arc<StateDb>,
    pub sink: Option<Arc<XavierEventSink>>,
}

/// Build the axum router for the event bus.
pub fn build_router(state: BusState) -> Router {
    Router::new()
        .route("/api/event", post(handle_event_http))
        .route("/api/events", get(list_events_http))
        .route("/healthz", get(healthz))
        .with_state(state)
}

/// `POST /api/event` — accept a BusEvent from any agent.
async fn handle_event_http(
    State(state): State<BusState>,
    Json(ev): Json<BusEvent>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let seq = handle_event(&state.db, &ev, state.sink.as_deref())
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    info!(
        agent = %ev.agent,
        event_type = %ev.event_type,
        run_id = ?ev.run_id,
        "bus event persisted (seq={})",
        seq
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "seq": seq,
        "ts": ev.ts,
    })))
}

/// `GET /api/events` — tail of recent bus events (chronological).
async fn list_events_http(
    State(state): State<BusState>,
    axum::extract::Query(params): axum::extract::Query<EventsQuery>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let limit = params.limit.unwrap_or(50).min(500);
    let mut events = state
        .db
        .recent_timeline(limit)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // recent_timeline returns DESC; reverse for chronological order.
    events.reverse();

    Ok(Json(serde_json::json!({
        "count": events.len(),
        "events": events,
    })))
}

#[derive(serde::Deserialize)]
struct EventsQuery {
    limit: Option<i64>,
}

/// `GET /healthz` — liveness probe.
async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok", "service": "gestalt-bus"}))
}

/// Run the bus HTTP server on `host:port` (default 127.0.0.1:8081).
///
/// Creates (or opens) the durable StateDb and an optional Xavier sink
/// (enabled when `XAVIER_TOKEN` is set), then serves until cancelled.
pub async fn serve(
    host: &str,
    port: u16,
    db_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state_db_path = db_path.map(std::path::PathBuf::from).unwrap_or_else(|| {
        home::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".gestalt")
            .join("state.db")
    });

    let db = Arc::new(StateDb::open(&state_db_path)?);

    // Xavier sink only when a token is configured (never panic).
    let sink = std::env::var("XAVIER_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|_| Arc::new(XavierEventSink::from_env()));

    if let Some(ref s) = sink {
        if s.is_available().await {
            info!("Bus → Xavier sink connected (Xavier :8006)");
        } else {
            warn!("Xavier not reachable — events will queue in StateDb (replay later)");
        }
    } else {
        warn!("XAVIER_TOKEN not set — bus running WITHOUT Xavier streaming");
    }

    let app = build_router(BusState { db, sink });
    let addr = format!("{}:{}", host, port);

    info!("Gestalt Event Bus listening on http://{addr}");
    println!("📡 Gestalt Event Bus on http://{addr}");
    println!("   POST /api/event  — push a BusEvent");
    println!("   GET  /api/events — tail recent events");
    println!("   GET  /healthz    — liveness");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ── Thinking loop: Ollama synthesizer + structural fallback ────────────────

/// Synthesizes cross-run insights via a local Ollama LLM (qwen3-coder).
///
/// Resilience (AGENTS.md §4): if Ollama is unreachable or the call fails,
/// `synthesize` falls back to a deterministic structural summary instead of
/// erroring — the thinking loop never crashes on LLM unavailability.
pub struct OllamaSynthesizer {
    endpoint: String,
    model: String,
}

impl OllamaSynthesizer {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
        }
    }

    /// Default: Ollama on localhost:11434 with qwen3-coder.
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into()),
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3-coder".into()),
        )
    }
}

#[async_trait::async_trait]
impl gestalt_router::thinking::InsightSynthesizer for OllamaSynthesizer {
    async fn synthesize(&self, executions: &[String]) -> Result<String, String> {
        let prompt = format!(
            "You are Xavier's thinking layer. Based on these recent agent executions:\n\n{}\n\n\
             Produce a concise insight with exactly 4 sections:\n\
             - PATTERNS: recurring behaviors or outcomes\n\
             - BLOCKERS: repeated failures or stalls\n\
             - DECISIONS: choices made and their rationale\n\
             - NEXT: recommended next steps\n\n\
             Keep under 200 words. No preamble.",
            executions.join("\n")
        );

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": prompt,
                "stream": false,
            }))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let body: serde_json::Value = r
                    .json()
                    .await
                    .map_err(|e| format!("Ollama response parse failed: {}", e))?;
                let text = body["response"].as_str().unwrap_or("").trim().to_string();
                if text.is_empty() {
                    Err("Ollama returned empty response".into())
                } else {
                    Ok(text)
                }
            },
            Ok(r) => {
                // Fallback: structural summary (deterministic, no LLM needed).
                warn!("Ollama returned {} — using structural fallback", r.status());
                Ok(structural_insight(executions))
            },
            Err(e) => {
                warn!("Ollama unreachable ({}) — using structural fallback", e);
                Ok(structural_insight(executions))
            },
        }
    }
}

/// Deterministic structural insight — patterns/blockers/decisions/next derived
/// from event metadata without any LLM call. Keeps the loop useful when the
/// local model is down.
fn structural_insight(executions: &[String]) -> String {
    let mut states = std::collections::HashMap::<String, usize>::new();
    for exec in executions {
        // Content format: "[event_type] agent run_id — summary"
        let head = exec.split_whitespace().next().unwrap_or("unknown");
        *states.entry(head.to_string()).or_insert(0) += 1;
    }
    let mut parts: Vec<String> = states
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect();
    parts.sort();
    format!(
        "PATTERNS: {} total executions ({})\nBLOCKERS: none detected (LLM offline)\nDECISIONS: n/a\nNEXT: re-run with Ollama online for deeper synthesis",
        executions.len(),
        parts.join(", ")
    )
}
