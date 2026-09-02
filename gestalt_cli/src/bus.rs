//! Gestalt Universal Event Bus — HTTP ingress + serve.
//!
//! Design: `docs/design/xavier-thinking-bus.md` (P0, section 4.1 + 4.2).
//!
//! Exposes:
//! - `POST /api/event`  — any agent pushes a [`BusEvent`] (fire-and-forget)
//! - `GET  /api/events` — filtered + paginated tail of recent bus events from StateDb (dashboard)
//!   Query params:
//!   - `agent`      (optional) filter by agent ID
//!   - `event_type` (optional, alias `type`) filter by event kind
//!   - `project`    (optional) filter by project name
//!   - `after_seq`  (optional) cursor sequence number for pagination
//!   - `limit`      (optional) maximum number of events to return (default 50, max 500)
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
        .route("/bus/event", post(handle_event_http))
        .route("/api/events", get(list_events_http))
        .route("/bus/events", get(list_events_http))
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/bus/health", get(healthz))
        .with_state(state)
}

/// `POST /api/event` — accept a BusEvent from any agent.
pub async fn handle_event_http(
    State(state): State<BusState>,
    Json(ev): Json<BusEvent>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let result = handle_event(&state.db, &ev, state.sink.as_deref())
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // WS publish hook: publish the BusEvent to the WS stream
    let ev_ws = gestalt_router::ws::gestalt_ws::BusEvent {
        agent: ev.agent.clone(),
        event_type: ev.event_type.clone(),
        run_id: ev.run_id.clone(),
        project: ev.project.clone(),
        state: ev.state.clone(),
        summary: ev.summary.clone(),
        metadata: ev.metadata.clone(),
        ts: ev.ts.clone(),
    };
    gestalt_router::ws::gestalt_ws::publish_to_all_adapters(&ev_ws);

    match result {
        Some(seq) => Ok(Json(serde_json::json!({
            "status": "ok",
            "seq": seq,
            "deduped": false,
            "ts": ev.ts,
        }))),
        None => {
            info!(
                agent = %ev.agent,
                event_type = %ev.event_type,
                "bus event deduplicated (skipped)"
            );
            Ok(Json(serde_json::json!({
                "status": "ok",
                "seq": null,
                "deduped": true,
                "ts": ev.ts,
            })))
        },
    }
}

/// `GET /api/events` — tail of recent bus events with filtering and cursor pagination.
pub async fn list_events_http(
    State(state): State<BusState>,
    axum::extract::Query(params): axum::extract::Query<EventsQuery>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let events = state
        .db
        .query_timeline(
            params.agent.as_deref(),
            params.event_type.as_deref(),
            params.project.as_deref(),
            params.after_seq,
            limit,
        )
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let next_seq = events.last().and_then(|e| e.seq);

    Ok(Json(serde_json::json!({
        "count": events.len(),
        "events": events,
        "next_seq": next_seq,
        "cursor": next_seq,
    })))
}

#[derive(serde::Deserialize, Clone)]
pub struct EventsQuery {
    pub agent: Option<String>,
    #[serde(alias = "type")]
    pub event_type: Option<String>,
    pub project: Option<String>,
    pub after_seq: Option<i64>,
    pub limit: Option<i64>,
}

/// `GET /healthz` — liveness probe.
async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok", "service": "gestalt-bus"}))
}

/// Run the bus HTTP server on `host:port` (default 127.0.0.1:8081).
///
/// Creates (or opens) the durable StateDb and an optional Xavier sink
/// (enabled when `XAVIER_TOKEN` is set), then serves until cancelled.
#[allow(dead_code)]
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
    println!("   POST /api/event (/bus/event)   — push a BusEvent");
    println!("   GET  /api/events (/bus/events) — tail recent events");
    println!("   GET  /health (/bus/health, /healthz) — liveness");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ── Thinking loop: deterministic structural synthesizer (no external LLM) ──

/// Deterministic insight synthesizer — derives PATTERNS/BLOCKERS/DECISIONS/NEXT
/// from the event corpus without any external LLM call. Zero dependencies:
/// no Ollama, no API keys, no network. This is the default and only
/// synthesizer for the thinking loop (decision 2026-08-05: no Ollama).
pub struct StructuralSynthesizer;

#[async_trait::async_trait]
impl gestalt_router::thinking::InsightSynthesizer for StructuralSynthesizer {
    async fn synthesize(&self, executions: &[String]) -> Result<String, String> {
        Ok(structural_insight(executions))
    }
}

/// Deterministic structural insight — patterns/blockers/decisions/next derived
/// from event content without any LLM call.
fn structural_insight(executions: &[String]) -> String {
    let mut states = std::collections::HashMap::<String, usize>::new();
    let mut agents = std::collections::HashMap::<String, usize>::new();
    let mut bus_events = 0usize;
    for exec in executions {
        // Bus events have the canonical "[event_type] agent run_id — summary"
        // shape (written by xavier_sink). Non-bus memories (session transcripts,
        // documents) are skipped so the insight reflects real bus activity.
        let trimmed = exec.trim();
        if !trimmed.starts_with('[') {
            continue;
        }
        bus_events += 1;
        let mut parts = trimmed.split_whitespace();
        let head = parts.next().unwrap_or("unknown").trim_matches(['[', ']']);
        *states.entry(head.to_string()).or_insert(0) += 1;
        if let Some(agent) = parts.next() {
            *agents.entry(agent.to_string()).or_insert(0) += 1;
        }
    }
    if bus_events == 0 {
        return format!(
            "PATTERNS: 0 bus events in window ({} memories retrieved, none from the bus)\nBLOCKERS: no bus traffic — is `gestalt bus serve` running and are agents pushing?\nDECISIONS: n/a\nNEXT: push events via event.py or gestalt run",
            executions.len()
        );
    }
    let mut state_parts: Vec<String> = states
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect();
    state_parts.sort();
    let mut agent_parts: Vec<String> = agents
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect();
    agent_parts.sort();
    format!(
        "PATTERNS: {} bus executions — by state [{}]; by agent [{}]\nBLOCKERS: none detected (deterministic analysis)\nDECISIONS: n/a\nNEXT: review recent executions for manual triage",
        bus_events,
        state_parts.join(", "),
        agent_parts.join(", ")
    )
}
