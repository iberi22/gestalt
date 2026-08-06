#[path = "../src/observe/artifact_ingest.rs"]
pub mod artifact_ingest;

use axum::{routing::get, Json, Router};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_artifact_ingest_parse_claude_line_real_fixture() {
    let fixture = r#"{"type":"assistant","isSideChain":false,"toolName":"Read","timestamp":"2026-08-06T12:00:00Z"}"#;
    let ev = artifact_ingest::parse_claude_line(fixture);
    assert!(ev.is_some());
    let ev = ev.unwrap();
    assert_eq!(ev.agent, "claude");
    assert_eq!(ev.event_type, "checkpoint");
    assert_eq!(ev.ts, "2026-08-06T12:00:00Z");
    assert_eq!(ev.summary, "Claude executed tool: Read");
    assert_eq!(ev.metadata["toolName"], "Read");
    assert_eq!(ev.metadata["isSideChain"], false);

    // Test with side chain true
    let fixture_side = r#"{"type":"assistant","isSideChain":true,"toolName":"Write","timestamp":"2026-08-06T12:05:00Z"}"#;
    let ev_side = artifact_ingest::parse_claude_line(fixture_side);
    assert!(ev_side.is_some());
    let ev_side = ev_side.unwrap();
    assert_eq!(ev_side.event_type, "tool_call");
    assert_eq!(ev_side.metadata["isSideChain"], true);
}

#[test]
fn test_artifact_ingest_parse_claude_line_malformed() {
    let malformed = r#"{"type":"assistant", "isSideChain": "#;
    let ev = artifact_ingest::parse_claude_line(malformed);
    assert!(ev.is_none()); // skipped, no panic

    let empty = "";
    let ev_empty = artifact_ingest::parse_claude_line(empty);
    assert!(ev_empty.is_none());
}

#[test]
fn test_artifact_ingest_offset_tracking() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("transcript.jsonl");

    // Write some lines initially
    let content_1 = r#"{"type":"assistant","isSideChain":false,"toolName":"Read","timestamp":"2026-08-06T12:00:00Z"}
{"type":"assistant","isSideChain":true,"toolName":"Write","timestamp":"2026-08-06T12:01:00Z"}
"#;
    fs::write(&file_path, content_1).unwrap();

    let mut offset = 0;
    // Initial tail should ingest everything
    let lines_1 = artifact_ingest::tail_file(&file_path, &mut offset).unwrap();
    assert_eq!(lines_1.len(), 2);
    assert_eq!(offset, content_1.len() as u64);

    // Re-ingest same content -> 0 new lines
    let lines_2 = artifact_ingest::tail_file(&file_path, &mut offset).unwrap();
    assert_eq!(lines_2.len(), 0);
    assert_eq!(offset, content_1.len() as u64);

    // Append new line
    let content_2 = r#"{"type":"assistant","isSideChain":false,"toolName":"Search","timestamp":"2026-08-06T12:02:00Z"}
"#;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&file_path)
        .unwrap();
    use std::io::Write;
    file.write_all(content_2.as_bytes()).unwrap();

    // Tail should only return the new line
    let lines_3 = artifact_ingest::tail_file(&file_path, &mut offset).unwrap();
    assert_eq!(lines_3.len(), 1);
    assert!(lines_3[0].contains("Search"));
    assert_eq!(offset, (content_1.len() + content_2.len()) as u64);

    // Truncate the file to verify offset reset
    fs::write(&file_path, content_2).unwrap();
    let lines_4 = artifact_ingest::tail_file(&file_path, &mut offset).unwrap();
    // Offset reset to 0, reads content_2 lines
    assert_eq!(lines_4.len(), 1);
    assert!(lines_4[0].contains("Search"));
    assert_eq!(offset, content_2.len() as u64);
}

#[test]
fn test_artifact_ingest_poll_hermes_sessions() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("sessions.db");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            state TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO sessions (id, name, state, created_at) VALUES (?, ?, ?, ?)",
        [
            "session-1",
            "Initialize workspace",
            "Running",
            "2026-08-06T12:00:00Z",
        ],
    )
    .unwrap();

    let mut last_rowid = 0;
    let events_1 = artifact_ingest::poll_hermes_sessions(&db_path, &mut last_rowid).unwrap();
    assert_eq!(events_1.len(), 1);
    assert_eq!(events_1[0].agent, "hermes");
    assert_eq!(events_1[0].event_type, "run_started");
    assert_eq!(events_1[0].run_id, Some("session-1".to_string()));
    assert_eq!(last_rowid, 1);

    // Poll again with updated rowid offset -> should find 0 new events
    let events_2 = artifact_ingest::poll_hermes_sessions(&db_path, &mut last_rowid).unwrap();
    assert_eq!(events_2.len(), 0);
    assert_eq!(last_rowid, 1);

    // Insert another session
    conn.execute(
        "INSERT INTO sessions (id, name, state, created_at) VALUES (?, ?, ?, ?)",
        [
            "session-2",
            "Code changes",
            "Success",
            "2026-08-06T12:10:00Z",
        ],
    )
    .unwrap();

    let events_3 = artifact_ingest::poll_hermes_sessions(&db_path, &mut last_rowid).unwrap();
    assert_eq!(events_3.len(), 1);
    assert_eq!(events_3[0].run_id, Some("session-2".to_string()));
    assert_eq!(last_rowid, 2);
}

#[test]
fn test_artifact_ingest_poll_jules_github_issues() {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let app = Router::new().route(
                "/repos/iberi22/gestalt-rust/issues",
                get(|| async {
                    Json(vec![
                        serde_json::json!({
                            "number": 101,
                            "title": "Fix bug in state",
                            "state": "open",
                            "updated_at": "2026-08-06T12:00:00Z",
                            "html_url": "https://github.com/iberi22/gestalt-rust/issues/101",
                            "labels": vec![serde_json::json!({ "name": "jules" })]
                        }),
                        serde_json::json!({
                            "number": 102,
                            "title": "Implement trait",
                            "state": "closed",
                            "updated_at": "2026-08-06T12:05:00Z",
                            "html_url": "https://github.com/iberi22/gestalt-rust/issues/102",
                            "labels": vec![
                                serde_json::json!({ "name": "jules" }),
                                serde_json::json!({ "name": "completed" })
                            ]
                        }),
                    ])
                }),
            );

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tx.send(addr).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });

    let addr = rx.recv().unwrap();
    std::env::set_var("GITHUB_API_URL", format!("http://{}", addr));

    let mut last_updated_at = String::new();
    let events = artifact_ingest::poll_jules_github_issues(
        "iberi22",
        "gestalt-rust",
        None,
        &mut last_updated_at,
    )
    .unwrap();

    assert_eq!(events.len(), 2);

    assert_eq!(events[0].agent, "jules");
    assert_eq!(events[0].event_type, "run_started");
    assert_eq!(events[0].state, Some("Running".to_string()));
    assert_eq!(events[0].run_id, Some("jules-issue-101".to_string()));

    assert_eq!(events[1].agent, "jules");
    assert_eq!(events[1].event_type, "run_finished");
    assert_eq!(events[1].state, Some("Success".to_string()));
    assert_eq!(events[1].run_id, Some("jules-issue-102".to_string()));

    assert_eq!(last_updated_at, "2026-08-06T12:05:00Z");

    // Poll again -> should skip since we use last_updated_at offset
    let events_empty = artifact_ingest::poll_jules_github_issues(
        "iberi22",
        "gestalt-rust",
        None,
        &mut last_updated_at,
    )
    .unwrap();
    assert_eq!(events_empty.len(), 0);
}
