use gestalt_state::StateDb;

#[test]
fn test_flight_recorder_interleaved() {
    let db = StateDb::open(":memory:").expect("Failed to open in-memory DB");

    let run_a = "run-A";
    let run_b = "run-B";

    db.create_run(run_a, r#"{"task": "task A"}"#).unwrap();
    db.create_run(run_b, r#"{"task": "task B"}"#).unwrap();

    // Push interleaved timeline events
    let _a1 = db.push_event(run_a, Some("agent-1"), "started", r#"{"idx": 1}"#).unwrap();
    let _b1 = db.push_event(run_b, Some("agent-2"), "started", r#"{"idx": 1}"#).unwrap();
    let _a2 = db.push_event(run_a, Some("agent-1"), "completed", r#"{"idx": 2}"#).unwrap();
    let _b2 = db.push_event(run_b, Some("agent-2"), "completed", r#"{"idx": 2}"#).unwrap();

    // Test timeline_by_run for run_a
    let timeline_a = db.timeline_by_run(run_a).expect("Failed to query run A");
    assert_eq!(timeline_a.len(), 2, "Should only have run A events");
    assert_eq!(timeline_a[0].event_type, "started");
    assert_eq!(timeline_a[1].event_type, "completed");
    assert_eq!(timeline_a[0].payload, r#"{"idx": 1}"#);
    assert_eq!(timeline_a[1].payload, r#"{"idx": 2}"#);

    // Test standalone helper timeline_by_run as well
    let timeline_a_standalone = gestalt_state::statedb::timeline_by_run(run_a, &db).expect("Failed to query run A with standalone helper");
    assert_eq!(timeline_a_standalone.len(), 2);

    // Test timeline_by_run for run_b
    let timeline_b = db.timeline_by_run(run_b).expect("Failed to query run B");
    assert_eq!(timeline_b.len(), 2, "Should only have run B events");
    assert_eq!(timeline_b[0].event_type, "started");
    assert_eq!(timeline_b[1].event_type, "completed");
    assert_eq!(timeline_b[0].payload, r#"{"idx": 1}"#);
    assert_eq!(timeline_b[1].payload, r#"{"idx": 2}"#);
}

#[test]
fn test_flight_recorder_empty_run_id() {
    let db = StateDb::open(":memory:").expect("Failed to open in-memory DB");

    // Test with empty string
    let timeline_empty = db.timeline_by_run("").expect("Failed on empty run_id");
    assert!(timeline_empty.is_empty(), "Empty run_id should return empty timeline");

    // Test with nonexistent run_id
    let timeline_nonexistent = db.timeline_by_run("nonexistent-id").expect("Failed on nonexistent run_id");
    assert!(timeline_nonexistent.is_empty(), "Nonexistent run_id should return empty timeline");
}
