//! Integration test for thin agent launcher (gestalt agent exec)
//!
//! describe("thin agent launcher execution flow")
//! it("should inject XAVIER_CONTEXT context")
//! it("should emit run_started and run_finished bus events")
//! it("should archive the run results to Xavier")

#[path = "../src/agent_wrapper.rs"]
mod agent_wrapper;

use agent_wrapper::{AgentWrapper, InMemoryVfs};
use axum::{routing::post, Json, Router};
use gestalt_state::statedb::StateDb;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

static ENV_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

fn get_env_mutex() -> &'static std::sync::Mutex<()> {
    ENV_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_agent_exec_trace_lifecycle() {
    // We synchronize environment modifications using our OnceLock Mutex
    let _env_guard = get_env_mutex().lock().unwrap();

    // Set up a clean temp directory for HOME to sandbox StateDb
    let temp_home = std::env::temp_dir().join(format!("agent_exec_home_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_home).unwrap();
    let old_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", &temp_home);

    // Track requests received by our mock server
    let received_events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let received_archives = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));

    let events_clone = received_events.clone();
    let archives_clone = received_archives.clone();

    // Build the mock router
    let app = Router::new()
        .route(
            "/api/event",
            post(move |Json(payload): Json<serde_json::Value>| {
                events_clone.lock().unwrap().push(payload);
                async { Json(serde_json::json!({ "status": "ok", "seq": 42 })) }
            }),
        )
        .route(
            "/v1/memories/search",
            post(|Json(_payload): Json<serde_json::Value>| async {
                Json(serde_json::json!({
                    "count": 1,
                    "results": [
                        {
                            "id": "mem-1",
                            "path": "test/path",
                            "snippet": "prior context snippet"
                        }
                    ]
                }))
            }),
        )
        .route(
            "/v1/memories",
            post(move |Json(payload): Json<serde_json::Value>| {
                archives_clone.lock().unwrap().push(payload);
                async { Json(serde_json::json!({ "status": "ok", "id": "archived-1" })) }
            }),
        );

    // Start mock server on a random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let mock_server_url = format!("http://{}", local_addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Configure env variables to point to mock server
    std::env::set_var("XAVIER_URL", &mock_server_url);
    std::env::set_var("XAVIER_TOKEN", "mock-token");
    std::env::set_var("GESTALT_PROJECT", "test-project");

    // Initialize AgentWrapper
    let vfs = Arc::new(InMemoryVfs::new());
    let run_id = "run-trace-123".to_string();
    let agent_id = "test-agent".to_string();

    // We run the printenv command to print XAVIER_CONTEXT
    let command = "printenv XAVIER_CONTEXT".to_string();

    let wrapper = AgentWrapper::new(vfs, agent_id, run_id, command);

    // Run execution with tracing
    let result = wrapper
        .execute_with_trace("fix bug in lib", Some("test-project"), Some(5))
        .await;
    assert!(
        result.is_ok(),
        "execute_with_trace failed: {:?}",
        result.err()
    );

    let (_edits, status, stdout, _stderr) = result.unwrap();
    assert!(status.success());
    assert!(
        stdout.contains("prior context snippet"),
        "Expected injected XAVIER_CONTEXT in stdout, but got: {}",
        stdout
    );

    // Give a split second for any background tokio spawns to complete
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Verify StateDb events
    let db_path = temp_home.join(".gestalt").join("state.db");
    let db = StateDb::open(&db_path).unwrap();
    let events_in_db = db.recent_timeline(10).unwrap();
    assert!(
        !events_in_db.is_empty(),
        "Events should be persisted in StateDb"
    );

    // Check if run_started and run_finished events were logged
    let start_logged = events_in_db
        .iter()
        .any(|e| e.payload.contains("run_started"));
    let finish_logged = events_in_db
        .iter()
        .any(|e| e.payload.contains("run_finished"));
    assert!(start_logged, "run_started not found in StateDb");
    assert!(finish_logged, "run_finished not found in StateDb");

    // Verify archives received
    let archives = received_archives.lock().unwrap();
    assert!(
        !archives.is_empty(),
        "Mock server should receive run archive"
    );
    let has_run_result = archives
        .iter()
        .any(|arch| arch.get("kind").and_then(|k| k.as_str()) == Some("run_result"));
    assert!(
        has_run_result,
        "Mock server did not receive run_result archive"
    );

    // Clean up environment and files
    if let Some(h) = old_home {
        std::env::set_var("HOME", h);
    } else {
        std::env::remove_var("HOME");
    }
    std::env::remove_var("XAVIER_URL");
    std::env::remove_var("XAVIER_TOKEN");
    std::env::remove_var("GESTALT_PROJECT");
    let _ = std::fs::remove_dir_all(&temp_home);
}
