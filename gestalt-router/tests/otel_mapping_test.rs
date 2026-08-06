use gestalt_router::event_bus::BusEvent;
use gestalt_router::otel::bus_event_to_otel_attributes;
use serde_json::json;

#[test]
fn test_run_finished_mapping() {
    let event = BusEvent::new("jules", "run_finished", "Execution complete")
        .with_run_id("run-12345")
        .with_state("Success")
        .with_metadata(json!({
            "llm": "gpt-4o",
            "provider": "openai",
            "decision": "rebuild"
        }));

    let otel = bus_event_to_otel_attributes(&event);

    // Verify gen_ai.agent.name
    assert_eq!(otel["gen_ai.agent.name"], json!("jules"));

    // Verify trace_id and gen_ai.conversation.id matches run_id
    assert_eq!(otel["trace_id"], json!("run-12345"));
    assert_eq!(otel["gen_ai.conversation.id"], json!("run-12345"));

    // Verify standard event_type mapping (run_finished is not special, so defaults to chat)
    assert_eq!(otel["span_type"], json!("chat"));

    // Verify llm mappings
    assert_eq!(otel["gen_ai.request.model"], json!("gpt-4o"));
    assert_eq!(otel["gen_ai.system"], json!("openai"));

    // Verify dynamic nested mapping
    assert_eq!(otel["gen_ai.decision"], json!("rebuild"));
}

#[test]
fn test_checkpoint_mapping() {
    let event = BusEvent::new("hermes", "checkpoint", "File modified")
        .with_run_id("run-abc")
        .with_metadata(json!({
            "tool_calls": 3,
            "gen_ai.already_prefixed": "yes"
        }));

    let otel = bus_event_to_otel_attributes(&event);

    // Verify checkpoint maps to execute_tool
    assert_eq!(otel["span_type"], json!("execute_tool"));

    // Verify other metadata and fields
    assert_eq!(otel["gen_ai.agent.name"], json!("hermes"));
    assert_eq!(otel["gen_ai.tool_calls"], json!(3));
    assert_eq!(otel["gen_ai.already_prefixed"], json!("yes"));
}

#[test]
fn test_run_started_mapping() {
    let event = BusEvent::new("gestalt", "run_started", "Orchestration sequence starting");

    let otel = bus_event_to_otel_attributes(&event);

    // Verify run_started maps to invoke_agent
    assert_eq!(otel["span_type"], json!("invoke_agent"));
    assert_eq!(otel["gen_ai.agent.name"], json!("gestalt"));
}

#[test]
fn test_unknown_event_type_fallback() {
    let event = BusEvent::new("external-bot", "some_crazy_new_event", "Something unknown happened");

    let otel = bus_event_to_otel_attributes(&event);

    // Verify it doesn't panic and falls back to chat
    assert_eq!(otel["span_type"], json!("chat"));
    assert_eq!(otel["gen_ai.agent.name"], json!("external-bot"));
    assert_eq!(otel["gen_ai.event.summary"], json!("Something unknown happened"));
}
