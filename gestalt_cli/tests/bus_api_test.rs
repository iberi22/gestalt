// Test describe: Gestalt Event Bus Query and Pagination Integration Tests
// it("should support filtering by agent, type, project and cursor pagination")
// it("should correctly handle limit query parameters")
// it("should return the next_seq/cursor pagination indicators")

#[path = "../src/bus.rs"]
pub mod bus;

use axum::extract::{Query, State};
use bus::{list_events_http, BusState, EventsQuery};
use gestalt_router::event_bus::BusEvent;
use gestalt_state::StateDb;
use std::sync::Arc;

#[tokio::test]
async fn it_should_query_events_with_filters_and_pagination() {
    // Open an in-memory database
    let db = Arc::new(StateDb::open(":memory:").unwrap());
    let state = BusState {
        db: db.clone(),
        sink: None,
    };

    // Seed events into StateDb
    let ev1 = BusEvent::new("agent-a", "run_started", "summary-1").with_project("project-x");
    let ev2 = BusEvent::new("agent-b", "checkpoint", "summary-2").with_project("project-y");
    let ev3 = BusEvent::new("agent-a", "run_finished", "summary-3").with_project("project-x");

    gestalt_router::event_bus::persist_event(&db, &ev1).unwrap();
    gestalt_router::event_bus::persist_event(&db, &ev2).unwrap();
    gestalt_router::event_bus::persist_event(&db, &ev3).unwrap();

    // 1. Get all events (no filters)
    let params = EventsQuery {
        agent: None,
        event_type: None,
        project: None,
        after_seq: None,
        limit: None,
    };
    let response = list_events_http(State(state.clone()), Query(params))
        .await
        .unwrap();
    let json = response.0;
    assert_eq!(json["count"].as_u64().unwrap(), 3);

    // 2. Filter by project
    let params = EventsQuery {
        agent: None,
        event_type: None,
        project: Some("project-x".to_string()),
        after_seq: None,
        limit: None,
    };
    let response = list_events_http(State(state.clone()), Query(params))
        .await
        .unwrap();
    let json = response.0;
    assert_eq!(json["count"].as_u64().unwrap(), 2);
    // Parse the payload strings inside events to inspect project
    let ev_payload_1: serde_json::Value =
        serde_json::from_str(json["events"][0]["payload"].as_str().unwrap()).unwrap();
    let ev_payload_2: serde_json::Value =
        serde_json::from_str(json["events"][1]["payload"].as_str().unwrap()).unwrap();
    assert_eq!(ev_payload_1["project"].as_str(), Some("project-x"));
    assert_eq!(ev_payload_2["project"].as_str(), Some("project-x"));

    // 3. Filter by agent
    let params = EventsQuery {
        agent: Some("agent-b".to_string()),
        event_type: None,
        project: None,
        after_seq: None,
        limit: None,
    };
    let response = list_events_http(State(state.clone()), Query(params))
        .await
        .unwrap();
    let json = response.0;
    assert_eq!(json["count"].as_u64().unwrap(), 1);
    assert_eq!(json["events"][0]["agent_id"].as_str(), Some("agent-b"));

    // 4. Cursor pagination (from sequence 0)
    let params = EventsQuery {
        agent: None,
        event_type: None,
        project: None,
        after_seq: Some(0),
        limit: Some(2),
    };
    let response = list_events_http(State(state.clone()), Query(params))
        .await
        .unwrap();
    let json = response.0;
    assert_eq!(json["count"].as_u64().unwrap(), 2);
    assert_eq!(json["events"][0]["seq"].as_i64().unwrap(), 1);
    assert_eq!(json["events"][1]["seq"].as_i64().unwrap(), 2);
    let cursor = json["next_seq"].as_i64().unwrap();
    assert_eq!(cursor, 2);

    // Fetch the next page after seq 2
    let params = EventsQuery {
        agent: None,
        event_type: None,
        project: None,
        after_seq: Some(cursor),
        limit: Some(2),
    };
    let response = list_events_http(State(state.clone()), Query(params))
        .await
        .unwrap();
    let json = response.0;
    assert_eq!(json["count"].as_u64().unwrap(), 1);
    assert_eq!(json["events"][0]["seq"].as_i64().unwrap(), 3);
}
