use axum::{
    extract::State,
    routing::post,
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use gestalt_core::application::agent::registry::{AgentRegistry, AgentEntry};
use gestalt_router::{
    agent::SubprocessRunner,
    event_bus::BusEvent,
    router::Router as GestaltRouter,
    run::{AgentSpec, RunSpec},
};
use gestalt_state::memstate::MemState;
use gestalt_core::application::agent::xavier::XavierClient;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use crate::bus::BusState;

static GLOBAL_MEM_STATE: OnceLock<MemState> = OnceLock::new();

pub fn get_mem_state() -> MemState {
    GLOBAL_MEM_STATE.get_or_init(MemState::new).clone()
}

#[derive(serde::Deserialize)]
pub struct TaskRequest {
    pub task: String,
    pub capabilities: Vec<String>,
    pub max_parallel: Option<usize>,
}

/// Router for the declarative task api.
pub fn router() -> Router<BusState> {
    Router::new().route("/api/task", post(handle_task_http))
}

/// Run the combined bus and task API HTTP server on `host:port`.
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

    let db = Arc::new(gestalt_state::StateDb::open(&state_db_path)?);

    let sink = std::env::var("XAVIER_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|_| Arc::new(gestalt_router::xavier_sink::XavierEventSink::from_env()));

    if let Some(ref s) = sink {
        if s.is_available().await {
            tracing::info!("Bus → Xavier sink connected (Xavier :8006)");
        } else {
            tracing::warn!("Xavier not reachable — events will queue in StateDb (replay later)");
        }
    } else {
        tracing::warn!("XAVIER_TOKEN not set — bus running WITHOUT Xavier streaming");
    }

    let bus_state = crate::bus::BusState { db: db.clone(), sink };
    let app = crate::bus::build_router(bus_state.clone()).merge(router().with_state(bus_state));

    let addr = format!("{}:{}", host, port);

    tracing::info!("Gestalt Event Bus + Task API listening on http://{addr}");
    println!("📡 Gestalt Event Bus + Task API on http://{addr}");
    println!("   POST /api/event  — push a BusEvent");
    println!("   GET  /api/events — tail recent events");
    println!("   POST /api/task   — declarative routing task API");
    println!("   GET  /healthz    — liveness");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Handler for POST /api/task
async fn handle_task_http(
    State(bus_state): State<BusState>,
    Json(req): Json<TaskRequest>,
) -> impl IntoResponse {
    let registry_path = "agent-registry.toml";
    let registry = match AgentRegistry::load(registry_path) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to load agent registry: {}", e)
                })),
            ).into_response();
        }
    };

    // Filter agents by capability intersection
    let matching_agents: Vec<AgentEntry> = registry
        .agents
        .iter()
        .filter(|agent| {
            req.capabilities.iter().all(|req_cap| agent.capabilities.contains(req_cap))
        })
        .cloned()
        .collect();

    if matching_agents.is_empty() {
        let available: Vec<serde_json::Value> = registry
            .agents
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "capabilities": a.capabilities,
                })
            })
            .collect();

        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "message": "No matching agents found for requested capabilities",
                "requested_capabilities": req.capabilities,
                "available_agents": available,
            })),
        ).into_response();
    }

    let run_id = uuid::Uuid::new_v4();
    let agent_ids: Vec<String> = matching_agents.iter().map(|a| a.name.clone()).collect();

    // Emit run_started event on the bus (traceability)
    let ev = BusEvent::new(
        "gestalt",
        "run_started",
        format!("Task API orchestration started: {}", req.task)
    )
    .with_run_id(run_id.to_string())
    .with_state("Running")
    .with_metadata(serde_json::json!({
        "agents": agent_ids,
        "task": req.task,
        "requested_by": "task_api",
    }));

    let db_clone = bus_state.db.clone();
    let sink_clone = bus_state.sink.clone();
    tokio::spawn(async move {
        let _ = gestalt_router::event_bus::handle_event(&db_clone, &ev, sink_clone.as_deref()).await;
    });

    // Wire matching agents to RunSpec and launch execution asynchronously
    let agent_specs: Vec<AgentSpec> = matching_agents
        .iter()
        .map(|a| AgentSpec {
            id: a.name.clone(),
            command: a.name.clone(),
            args: Vec::new(),
            allowed_paths: None,
            env: None,
            capabilities: a.capabilities.clone(),
        })
        .collect();

    let max_parallel = req.max_parallel.unwrap_or(4);
    let run_spec = RunSpec {
        base_ref: "main".to_string(),
        task: req.task.clone(),
        agents: agent_specs,
        max_parallel,
        timeout: 300,
        push: false,
        integration_branch: None,
    };

    let db = bus_state.db.clone();
    let xavier_client = XavierClient::from_env();

    tokio::spawn(async move {
        let runner = SubprocessRunner::new(Duration::from_secs(300));
        let mut gestalt_router = GestaltRouter::new(
            None,
            Arc::new(runner),
            db,
            get_mem_state(),
            None,
            None,
        );
        if xavier_client.is_available().await {
            gestalt_router = gestalt_router.with_xavier(xavier_client);
        }
        let _ = gestalt_router.execute(run_spec).await;
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "run_id": run_id.to_string(),
            "agents": agent_ids,
        })),
    ).into_response()
}
