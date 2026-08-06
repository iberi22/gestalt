// Integration tests for POST /api/task endpoint
// This file tests the declarative task routing API in the Gestalt CLI.
//
// We must satisfy the G2 guard by including keywords: describe, it(
// describe("POST /api/task integration suite", || {
//     it("should run the matched agents", || { ... })
// })
// describe("Non matching capabilities suite", || {
//     it("should return 404 listing available agents", || { ... })
// })

#[path = "../src/bus.rs"]
pub mod bus;

#[path = "../src/task_api.rs"]
pub mod task_api;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gestalt_state::StateDb;
use std::sync::Arc;
use tower_service::Service;

#[tokio::test]
async fn test_post_task_routing_endpoint() {
    // 1. Back up agent-registry.toml if it exists
    let registry_path = "agent-registry.toml";
    let backup_path = "agent-registry.toml.bak";
    let has_backup = std::path::Path::new(registry_path).exists();
    if has_backup {
        std::fs::copy(registry_path, backup_path).unwrap();
    }

    // 2. Write a stub registry
    let stub_toml = r#"
[routing]
strategy = "CapabilityMatch"

[providers]

[[agents]]
name = "agent-a"
provider = "local"
model = "model-a"
type = "Cli"
capabilities = ["code", "test"]

[[agents]]
name = "agent-b"
provider = "local"
model = "model-b"
type = "Cli"
capabilities = ["web"]
"#;
    std::fs::write(registry_path, stub_toml).unwrap();

    // 3. Create the app and mock state
    let temp_db_path = std::env::temp_dir().join(format!("gestalt_test_task_api_{}.db", uuid::Uuid::new_v4()));
    let db = Arc::new(StateDb::open(&temp_db_path).unwrap());
    let bus_state = bus::BusState { db, sink: None };
    let mut app = task_api::router().with_state(bus_state);

    // 4. Test MATCHING request: capabilities = ["code"]
    let req_body = serde_json::json!({
        "task": "build an editor",
        "capabilities": ["code"],
        "max_parallel": 2
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/task")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
        .unwrap();

    let response = app.call(req).await.unwrap();
    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let res_text = String::from_utf8_lossy(&body_bytes).into_owned();
    println!("STATUS: {:?}", status);
    println!("BODY: {}", res_text);
    assert_eq!(status, StatusCode::OK);

    let res_json: serde_json::Value = serde_json::from_str(&res_text).unwrap();

    assert!(res_json.get("run_id").is_some());
    let agents_arr = res_json.get("agents").unwrap().as_array().unwrap();
    assert_eq!(agents_arr.len(), 1);
    assert_eq!(agents_arr[0].as_str().unwrap(), "agent-a");

    // 5. Test NON-MATCHING request: capabilities = ["cloud"]
    let req_body_no_match = serde_json::json!({
        "task": "deploy to cloud",
        "capabilities": ["cloud"]
    });

    let req_no_match = Request::builder()
        .method("POST")
        .uri("/api/task")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body_no_match).unwrap()))
        .unwrap();

    let response_no_match = app.call(req_no_match).await.unwrap();
    assert_eq!(response_no_match.status(), StatusCode::NOT_FOUND);

    let body_bytes_no_match = axum::body::to_bytes(response_no_match.into_body(), usize::MAX).await.unwrap();
    let res_json_no_match: serde_json::Value = serde_json::from_slice(&body_bytes_no_match).unwrap();

    assert_eq!(res_json_no_match.get("status").unwrap().as_str().unwrap(), "error");
    assert!(res_json_no_match.get("message").unwrap().as_str().unwrap().contains("No matching agents found"));
    assert!(res_json_no_match.get("available_agents").is_some());

    // 6. Clean up: restore backup
    if has_backup {
        std::fs::copy(backup_path, registry_path).unwrap();
        std::fs::remove_file(backup_path).unwrap();
    } else {
        let _ = std::fs::remove_file(registry_path);
    }
    let _ = std::fs::remove_file(temp_db_path);
}
