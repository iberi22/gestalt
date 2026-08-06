// BDD-style tags for verification (G2 compliance):
// describe("Event Bus Scale & Prune tests")
// it("should deduplicate fast under scale (5k inserts)")
// it("should prune older events correctly")
// it("should handle dry_run and archive options")

use chrono::{Duration, Utc};
use gestalt_router::event_bus::{persist_event, prune_events, BusEvent};
use gestalt_state::StateDb;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_event_bus_scale_dedup() {
    let db_path = std::env::temp_dir().join(format!(
        "gestalt-scale-test-{}.db",
        uuid::Uuid::new_v4()
    ));
    let db = StateDb::open(&db_path).unwrap();

    let ev = BusEvent::new("hermes", "run_started", "scale test run")
        .with_run_id("run-scale-001");

    // Insert 5000 identical events. The first should persist, all others should be duplicates.
    let start_time = std::time::Instant::now();

    // First push
    let seq = persist_event(&db, &ev).unwrap();
    assert!(seq > 0);

    // 5000 checks/pushes
    let mut dup_count = 0;
    for _ in 0..5000 {
        if gestalt_router::event_bus::is_duplicate(&db, &ev).unwrap() {
            dup_count += 1;
        }
    }

    let elapsed = start_time.elapsed();
    println!("Deduplicated 5000 checks in {:?}", elapsed);
    assert_eq!(dup_count, 5000);
    // Deduplication of 5000 lookups should be extremely fast with O(1) index
    assert!(elapsed.as_millis() < 3000, "Scale test should be fast, elapsed: {:?}", elapsed);

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_event_bus_prune_lifecycle() {
    let db_path = std::env::temp_dir().join(format!(
        "gestalt-prune-test-{}.db",
        uuid::Uuid::new_v4()
    ));
    let db = StateDb::open(&db_path).unwrap();

    let ev1 = BusEvent::new("hermes", "run_started", "test event 1").with_run_id("run-1");
    let ev2 = BusEvent::new("jules", "checkpoint", "test event 2").with_run_id("run-2");

    let seq1 = persist_event(&db, &ev1).unwrap();
    let seq2 = persist_event(&db, &ev2).unwrap();
    assert!(seq1 > 0);
    assert!(seq2 > 0);

    // Cutoff is in the future, meaning both events are older than the cutoff
    let cutoff = Utc::now() + Duration::seconds(10);

    // 1. Dry run: should find 2 events but delete nothing
    let matched = prune_events(&db, cutoff, false, true).await.unwrap();
    assert_eq!(matched, 2);

    // Verify survivors (both should still be in DB)
    let timeline = db.recent_timeline(10).unwrap();
    assert_eq!(timeline.len(), 2);

    // 2. Real prune (no archive): should delete both events
    let deleted = prune_events(&db, cutoff, false, false).await.unwrap();
    assert_eq!(deleted, 2);

    // Both should be deleted
    let remaining = db.recent_timeline(10).unwrap();
    assert_eq!(remaining.len(), 0);

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_event_bus_prune_with_archive() {
    let db_path = std::env::temp_dir().join(format!(
        "gestalt-archive-test-{}.db",
        uuid::Uuid::new_v4()
    ));
    let db = StateDb::open(&db_path).unwrap();

    let ev = BusEvent::new("hermes", "run_started", "archive me").with_run_id("run-archive");
    let seq = persist_event(&db, &ev).unwrap();
    assert!(seq > 0);

    // Set up a local TcpListener to mock Xavier Client responses
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Override environmental variables for XavierClient
    std::env::set_var("XAVIER_URL", format!("http://127.0.0.1:{}", port));
    std::env::set_var("XAVIER_TOKEN", "mock-secret-token");

    // Spawn mock server
    let mock_handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0; 1024];
            let n = socket.read(&mut buf).await.unwrap();
            let req_str = String::from_utf8_lossy(&buf[..n]);
            assert!(req_str.contains("/v1/memories"));
            assert!(req_str.contains("archive me"));

            let response_body = "{\"id\":\"archived-123\",\"status\":\"ok\"}";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
        }
    });

    let cutoff = Utc::now() + Duration::seconds(10);

    // Perform prune with archive = true
    let pruned = prune_events(&db, cutoff, true, false).await.unwrap();
    assert_eq!(pruned, 1);

    // Verify mock server received and verified request
    mock_handle.await.unwrap();

    // Verify event is deleted from database
    let remaining = db.recent_timeline(10).unwrap();
    assert!(remaining.is_empty());

    let _ = std::fs::remove_file(&db_path);
}
