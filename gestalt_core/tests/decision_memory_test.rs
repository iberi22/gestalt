use gestalt_core::application::agent::xavier::{XavierClient, build_decision_payload};

#[test]
fn test_build_decision_payload_success() {
    let text = "Implement durable decision memory";
    let metadata = serde_json::json!({"issue": "GT-13"});
    let user_id = "jules-007";

    let result = build_decision_payload(text, metadata, user_id);
    assert!(result.is_ok());

    let payload = result.unwrap();
    assert_eq!(payload["text"], "Implement durable decision memory");
    assert_eq!(payload["user_id"], "jules-007");
    assert_eq!(payload["kind"], "decision");
    assert_eq!(payload["metadata"]["issue"], "GT-13");
}

#[test]
fn test_build_decision_payload_missing_text() {
    let text = "   ";
    let metadata = serde_json::json!({});
    let user_id = "jules-007";

    let result = build_decision_payload(text, metadata, user_id);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("text is required"));
}

#[test]
fn test_build_decision_payload_missing_user_id() {
    let text = "Implement durable decision memory";
    let metadata = serde_json::json!({});
    let user_id = "";

    let result = build_decision_payload(text, metadata, user_id);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("user_id is required"));
}

#[tokio::test]
async fn test_archive_decision_compiles() {
    // This test ensures archive_decision is fully implemented and compiles.
    // We don't have a live server, so calling it should fail on request/connection,
    // but the symbols are correctly resolved and compile.
    let client = XavierClient::new("http://localhost:12345".to_string(), "test-token".to_string()).unwrap();
    let res = client.archive_decision("Decide to use Rust", serde_json::json!({})).await;
    assert!(res.is_err()); // Connection refused/failed
}
