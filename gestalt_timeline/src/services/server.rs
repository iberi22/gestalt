use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, Path, State,
    },
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::models::TaskStatus;
use crate::services::{
    AgentRuntime, AgentService, ProjectService, TaskService, TimelineService, WatchService,
}; // Import TaskStatus

#[derive(Clone)]
pub struct AppState {
    pub runtime: AgentRuntime,
    pub timeline: TimelineService,
    pub agent: AgentService,
    pub project: ProjectService,
    pub task: TaskService,
    pub _watch: WatchService,
}

#[derive(Deserialize)]
pub struct OrchestrateRequest {
    pub goal: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub agent_id: Option<String>,
}

#[derive(Serialize)]
pub struct OrchestrateResponse {
    pub message: String,
    pub task_id: String,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub project: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub description: Option<String>,
    pub status: Option<String>, // "todo", "running", "completed", "cancelled"
}

#[derive(Deserialize)]
pub struct ScheduleTaskRequest {
    pub time: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct SetModeRequest {
    pub mode: String, // "build" or "plan"
}

#[derive(serde::Serialize)]
pub struct ModeResponse {
    pub mode: String,
    pub is_read_only: bool,
}

/// Start the Agent REST API server.
pub async fn start_server(
    runtime: AgentRuntime,
    timeline: TimelineService,
    agent: AgentService,
    project: ProjectService,
    task: TaskService,
    watch: WatchService,
    port: u16,
) -> anyhow::Result<()> {
    let state = AppState {
        runtime,
        timeline,
        agent,
        project,
        task,
        _watch: watch,
    };

    let app = Router::new()
        .route("/orchestrate", post(run_orchestration))
        .route("/chat", post(chat_endpoint))
        .route("/timeline", get(get_timeline))
        .route("/agents", get(get_agents))
        .route("/projects", get(get_projects).post(create_project))
        .route("/projects/:id", delete(delete_project))
        .route("/tasks", get(get_tasks).post(create_task))
        .route("/tasks/:id", put(update_task).delete(delete_task))
        .route("/tasks/:id/run", post(run_task_endpoint))
        .route("/tasks/:id/schedule", post(schedule_task_endpoint))
        .route("/health", get(health_check))
        .route("/config/mode", get(get_agent_mode).post(set_agent_mode)) // Agent mode toggle
        .route("/stream", get(ws_handler))
        .layer(middleware::from_fn(auth_middleware))
        .layer(CorsLayer::permissive()) // Allow Flutter app to access
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("🚀 Agent Server listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Security Middleware that checks GESTALT_API_TOKEN
async fn auth_middleware(
    headers: HeaderMap,
    req: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected_token = std::env::var("GESTALT_API_TOKEN").unwrap_or_default();
    if expected_token.is_empty() {
        return Ok(next.run(req).await);
    }

    let mut authorized = false;

    // Check Header
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str == format!("Bearer {}", expected_token) {
                authorized = true;
            }
        }
    }

    // Check Query Param
    if let Some(query) = req.uri().query() {
        if query.contains(&format!("token={}", expected_token)) {
            authorized = true;
        }
    }

    if authorized {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// WebSocket Handler for UI Real-Time Streaming
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut last_check = Utc::now();
    let poll_interval = tokio::time::Duration::from_millis(500);

    // Initial fetch to populate UI (last 2 hours)
    if let Ok(initial_events) = state.timeline.get_timeline(Some("2h")).await {
        for event in initial_events.into_iter().rev() {
            if let Ok(json) = serde_json::to_string(&event) {
                if socket.send(Message::Text(json)).await.is_err() {
                    return;
                }
            }
        }
    }

    // Stream loop
    loop {
        // Send a ping to detect disconnections
        if socket.send(Message::Ping(vec![])).await.is_err() {
            break;
        }

        if let Ok(events) = state.timeline.get_events_since(last_check).await {
            for event in events {
                let ts_utc = event.timestamp.0;
                if ts_utc > last_check {
                    last_check = ts_utc;
                }

                if let Ok(json) = serde_json::to_string(&event) {
                    if socket.send(Message::Text(json)).await.is_err() {
                        return;
                    }
                }
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

/// Handler: Trigger autonomous loop
async fn run_orchestration(
    State(state): State<AppState>,
    Json(payload): Json<OrchestrateRequest>,
) -> (StatusCode, Json<OrchestrateResponse>) {
    info!("📥 Received orchestration request: {}", payload.goal);

    let runtime = state.runtime.clone();
    let goal = payload.goal.clone();

    // Spawn background task
    tokio::spawn(async move {
        match runtime.run_loop(&goal).await {
            Ok(_) => info!("✅ Background orchestration completed for: {}", goal),
            Err(e) => info!("❌ Background orchestration failed: {}", e),
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(OrchestrateResponse {
            message: "Orchestration started".to_string(),
            task_id: "background-task".to_string(), // TODO: meaningful ID
        }),
    )
}

/// Handler: Chat and trigger orchestration
async fn chat_endpoint(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> (StatusCode, Json<OrchestrateResponse>) {
    info!("💬 Received chat message: {}", payload.message);

    let agent_id = payload.agent_id.unwrap_or_else(|| "user".to_string());

    // Record the user's message in the timeline
    let event = crate::models::TimelineEvent::new(&agent_id, crate::models::EventType::ChatMessage)
        .with_payload(serde_json::json!({
            "text": payload.message,
            "sender": agent_id
        }));

    let _ = state.timeline.record_event(event).await;

    // Trigger the agent's autonomous loop with the user's message as a goal
    let runtime = state.runtime.clone();
    let goal = payload.message.clone();

    tokio::spawn(async move {
        match runtime.run_loop(&goal).await {
            Ok(_) => info!("✅ Chat-triggered orchestration completed for: {}", goal),
            Err(e) => info!("❌ Chat-triggered orchestration failed: {}", e),
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(OrchestrateResponse {
            message: "Chat received, agent responding...".to_string(),
            task_id: "chat-task".to_string(),
        }),
    )
}

/// Handler: Get timeline events (pollable)
async fn get_timeline(State(state): State<AppState>) -> Json<Vec<crate::models::TimelineEvent>> {
    let events = state.timeline.get_timeline(None).await.unwrap_or_default();
    Json(events)
}

/// Handler: Get agents status
async fn get_agents(State(state): State<AppState>) -> Json<Vec<crate::services::Agent>> {
    let agents = state.agent.list_agents().await.unwrap_or_default();
    Json(agents)
}

/// Handler: Get all projects
async fn get_projects(State(state): State<AppState>) -> Json<Vec<crate::models::Project>> {
    let projects = state.project.list_projects().await.unwrap_or_default();
    Json(projects)
}

/// Handler: Create project
async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<CreateProjectRequest>,
) -> (StatusCode, Json<Option<crate::models::Project>>) {
    // Hardcoded agent ID for now since we lack auth middleware on API
    match state
        .project
        .create_project(&payload.name, "system-api")
        .await
    {
        Ok(project) => (StatusCode::CREATED, Json(Some(project))),
        Err(e) => {
            info!("Failed to create project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

/// Handler: Delete project
async fn delete_project(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    match state.project.delete_project(&id, "system-api").await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(e) => {
            info!("Failed to delete project: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Handler: Get all tasks (optionally filtered by project query param - simplified for now)
async fn get_tasks(State(state): State<AppState>) -> Json<Vec<crate::models::Task>> {
    // List all tasks by iterating projects (inefficient but works for MVP) or adding list_all to TaskService
    // Assuming we added list_tasks(None) -> all tasks support in TaskService which we did!
    match state.task.list_tasks(None).await {
        Ok(tasks) => Json(tasks),
        Err(_) => Json(vec![]),
    }
}

/// Handler: Create task
async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> (StatusCode, Json<Option<crate::models::Task>>) {
    match state
        .task
        .create_task(&payload.project, &payload.description, "system-api")
        .await
    {
        Ok(task) => (StatusCode::CREATED, Json(Some(task))),
        Err(e) => {
            info!("Failed to create task: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

/// Handler: Update task
async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTaskRequest>,
) -> (StatusCode, Json<Option<crate::models::Task>>) {
    let status = payload
        .status
        .and_then(|s| match s.to_lowercase().as_str() {
            "todo" => Some(TaskStatus::Pending),
            "running" | "inprogress" => Some(TaskStatus::Running),
            "completed" | "done" => Some(TaskStatus::Completed),
            "cancelled" | "failed" => Some(TaskStatus::Cancelled),
            _ => None,
        });

    match state
        .task
        .update_task(&id, payload.description, status, "system-api")
        .await
    {
        Ok(task) => (StatusCode::OK, Json(Some(task))),
        Err(e) => {
            info!("Failed to update task: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

/// Handler: Delete task
async fn delete_task(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    match state.task.delete_task(&id, "system-api").await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(e) => {
            info!("Failed to delete task: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Handler: Run task
async fn run_task_endpoint(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    match state.task.run_task(&id, "system-api").await {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            info!("Failed to run task: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Handler: Schedule task
async fn schedule_task_endpoint(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<ScheduleTaskRequest>,
) -> StatusCode {
    match state
        .task
        .schedule_task(&id, payload.time, "system-api")
        .await
    {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            info!("Failed to schedule task: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Handler: Simple health check
async fn health_check() -> StatusCode {
    StatusCode::OK
}

/// Handler: Get current agent mode
async fn get_agent_mode(State(_state): State<AppState>) -> Json<ModeResponse> {
    // For now, return default. In production, this would read from AgentOrchestrator state.
    Json(ModeResponse {
        mode: "build".to_string(),
        is_read_only: false,
    })
}

/// Handler: Set agent mode
async fn set_agent_mode(
    State(_state): State<AppState>,
    Json(payload): Json<SetModeRequest>,
) -> (StatusCode, Json<ModeResponse>) {
    let is_read_only = payload.mode.to_lowercase() == "plan";
    info!(
        "Agent mode set to: {} (read_only: {})",
        payload.mode, is_read_only
    );
    (
        StatusCode::OK,
        Json(ModeResponse {
            mode: payload.mode,
            is_read_only,
        }),
    )
}
