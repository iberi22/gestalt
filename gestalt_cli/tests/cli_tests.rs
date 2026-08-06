use std::process::Command;

#[test]
fn test_help_command() {
    let output = Command::new("cargo")
        .args(["run", "-p", "gestalt_cli", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OpenClaw ↔ Gestalt Bridge CLI"));
    assert!(stdout.contains("repl"));
}

#[test]
fn test_status_offline() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "gestalt_cli",
            "--",
            "status",
            "--url",
            "http://127.0.0.1:65535",
        ])
        .output()
        .expect("Failed to execute command");

    // Should fail because nothing is listening on that port
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Check for some indicators of failure.
    // Since we're using tracing, it might look different than a simple println.
    // But we know it should return an error.
    assert!(!combined.is_empty());
}

#[test]
fn test_version_flag() {
    let bin_path = env!("CARGO_BIN_EXE_gestalt_cli");
    let output = Command::new(bin_path)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("gestalt {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(&expected));
}

#[test]
fn test_doctor_command() {
    let bin_path = env!("CARGO_BIN_EXE_gestalt_cli");
    let output = Command::new(bin_path)
        .arg("doctor")
        .output()
        .expect("Failed to execute command");

    // The doctor command might succeed or fail depending on if services are offline/online.
    // That's fine, but let's check that the output contains the diagnostic checks we implemented!
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(combined.contains("Running Gestalt Doctor Environment Check"));
    assert!(combined.contains("Xavier reachability"));
    assert!(combined.contains("StateDb open"));
    assert!(combined.contains("Agent registry parse"));
    assert!(combined.contains("Bus serve reachability"));
    assert!(combined.contains("Verdict:"));
}

#[tokio::test]
async fn test_thinking_list_and_approve_with_stub_xavier() {
    use axum::{
        routing::{get, post},
        Json, Router,
    };

    let app = Router::new()
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({"status": "ok"})) }),
        )
        .route(
            "/v1/memories/search",
            post(|Json(_body): Json<serde_json::Value>| async move {
                Json(serde_json::json!({
                    "count": 1,
                    "results": [
                        {
                            "id": "insight-123",
                            "path": "gestalt/thinking/2026-08-05",
                            "content": "This is a synthesized pattern of 5 execution runs.",
                            "snippet": "This is a synthesized pattern",
                            "score": 0.95,
                            "metadata": {
                                "kind": "insight"
                            }
                        }
                    ]
                }))
            }),
        )
        .route(
            "/v1/memories",
            post(|Json(_body): Json<serde_json::Value>| async move {
                Json(serde_json::json!({
                    "id": "decision-456",
                    "status": "ok"
                }))
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let cli_bin = env!("CARGO_BIN_EXE_gestalt_cli");

    // 1. Test 'thinking list'
    let output_list = tokio::process::Command::new(cli_bin)
        .envs([
            ("XAVIER_URL", format!("http://{}", addr)),
            ("XAVIER_TOKEN", "test-token".to_string()),
        ])
        .args(["thinking", "list", "--recent", "--limit", "3"])
        .output()
        .await
        .expect("Failed to execute command");

    let stdout_list = String::from_utf8_lossy(&output_list.stdout);
    let stderr_list = String::from_utf8_lossy(&output_list.stderr);
    assert!(
        output_list.status.success(),
        "Command failed with status: {:?}\nstdout: {}\nstderr: {}",
        output_list.status,
        stdout_list,
        stderr_list
    );
    assert!(stdout_list.contains("insight-123"));
    assert!(stdout_list.contains("2026-08-05"));
    assert!(stdout_list.contains("This is a synthesized pattern"));

    // 2. Test 'thinking approve --dry-run'
    let output_approve_dry = tokio::process::Command::new(cli_bin)
        .envs([
            ("XAVIER_URL", format!("http://{}", addr)),
            ("XAVIER_TOKEN", "test-token".to_string()),
        ])
        .args(["thinking", "approve", "--id", "insight-123", "--dry-run"])
        .output()
        .await
        .expect("Failed to execute command");

    assert!(output_approve_dry.status.success());
    let stdout_approve_dry = String::from_utf8_lossy(&output_approve_dry.stdout);
    assert!(stdout_approve_dry.contains("[dry-run]"));
    assert!(stdout_approve_dry.contains("insight-123"));
    assert!(stdout_approve_dry.contains("gestalt/decisions/2026-08-05"));

    // 3. Test 'thinking approve' real run
    let output_approve_real = tokio::process::Command::new(cli_bin)
        .envs([
            ("XAVIER_URL", format!("http://{}", addr)),
            ("XAVIER_TOKEN", "test-token".to_string()),
        ])
        .args(["thinking", "approve", "--id", "insight-123"])
        .output()
        .await
        .expect("Failed to execute command");

    assert!(output_approve_real.status.success());
    let stdout_approve_real = String::from_utf8_lossy(&output_approve_real.stdout);
    assert!(stdout_approve_real.contains("Decision promoted successfully!"));
    assert!(stdout_approve_real.contains("decision-456"));
}
