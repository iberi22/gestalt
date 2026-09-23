use chrono::Utc;
use gestalt_bus::{AgentEvent, TelemetryEmitter};
use serde_json::json;

#[test]
fn test_step_progress_event_serialization() {
    let now = Utc::now();
    let event = AgentEvent::StepProgress {
        step: 2,
        total_steps: 5,
        message: "Processing step 2".to_string(),
        timestamp: now,
    };

    let serialized = serde_json::to_string(&event).expect("Serialization failed");
    let value: serde_json::Value = serde_json::from_str(&serialized).expect("Deserialization to Value failed");

    assert_eq!(value["event_type"], "step_progress");
    assert_eq!(value["step"], 2);
    assert_eq!(value["total_steps"], 5);
    assert_eq!(value["message"], "Processing step 2");

    let deserialized: AgentEvent = serde_json::from_str(&serialized).expect("Deserialization to AgentEvent failed");
    assert_eq!(deserialized, event);
}

#[test]
fn test_tool_invoked_event_serialization() {
    let now = Utc::now();
    let event = AgentEvent::ToolInvoked {
        tool: "read_file".to_string(),
        input: json!({"path": "src/main.rs"}),
        status: "success".to_string(),
        timestamp: now,
    };

    let serialized = serde_json::to_string(&event).expect("Serialization failed");
    let value: serde_json::Value = serde_json::from_str(&serialized).expect("Deserialization to Value failed");

    assert_eq!(value["event_type"], "tool_invoked");
    assert_eq!(value["tool"], "read_file");
    assert_eq!(value["input"]["path"], "src/main.rs");
    assert_eq!(value["status"], "success");

    let deserialized: AgentEvent = serde_json::from_str(&serialized).expect("Deserialization to AgentEvent failed");
    assert_eq!(deserialized, event);
}

#[test]
fn test_error_captured_event_serialization() {
    let now = Utc::now();
    let event = AgentEvent::ErrorCaptured {
        error: "File not found".to_string(),
        code: Some("ENOENT".to_string()),
        timestamp: now,
    };

    let serialized = serde_json::to_string(&event).expect("Serialization failed");
    let value: serde_json::Value = serde_json::from_str(&serialized).expect("Deserialization to Value failed");

    assert_eq!(value["event_type"], "error_captured");
    assert_eq!(value["error"], "File not found");
    assert_eq!(value["code"], "ENOENT");

    let deserialized: AgentEvent = serde_json::from_str(&serialized).expect("Deserialization to AgentEvent failed");
    assert_eq!(deserialized, event);
}

#[test]
fn test_telemetry_emitter() {
    let emitter = TelemetryEmitter::new();
    let event = AgentEvent::StepProgress {
        step: 1,
        total_steps: 1,
        message: "Done".to_string(),
        timestamp: Utc::now(),
    };

    let res = emitter.emit(&event);
    assert!(res.is_ok());
    let emitted_str = res.unwrap();
    assert!(emitted_str.contains("\"event_type\":\"step_progress\""));
}
