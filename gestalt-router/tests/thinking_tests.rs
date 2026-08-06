//! Integration and threshold tests for Xavier Thinking Loop gating policies.
//!
//! describe: ThinkingLoop gating policy tests
//! it("should not run with 0 executions")
//! it("should not run with 2 executions")
//! it("should run with 3 executions")
//! it("should run with 5 executions")
//! it("should filter out executions before the last insight date")

use gestalt_core::application::agent::xavier::XavierClient;
use gestalt_router::thinking::{ThinkingLoop, MIN_EXECUTIONS};
use gestalt_state::StateDb;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct TempDb {
    db: StateDb,
    _path: std::path::PathBuf,
}

impl TempDb {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("gestalt_thinking_test_{}.db", uuid::Uuid::new_v4()));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let db = StateDb::open(&path).unwrap();
        Self { db, _path: path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self._path);
    }
}

async fn spawn_mock_xavier(date_str: &'static str) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        // Handle connections in a loop to allow multiple requests
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                let mut bytes_read = 0;
                while bytes_read < buf.len() {
                    if let Ok(n) = socket.read(&mut buf[bytes_read..]).await {
                        if n == 0 {
                            break;
                        }
                        bytes_read += n;
                        let request = String::from_utf8_lossy(&buf[..bytes_read]);
                        if request.contains("\r\n\r\n") {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                let json_resp = format!(
                    r#"{{"count":1,"results":[{{"id":"1","path":"gestalt/thinking/{}","snippet":"test insight","score":1.0,"metadata":{{}}}}]}}"#,
                    date_str
                );

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    json_resp.len(),
                    json_resp
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    (port, handle)
}

#[tokio::test]
async fn test_should_run_thresholds_offline() {
    // describe: ThinkingLoop offline threshold checks
    // it("should return false with < 3 executions and true with >= 3 executions when Xavier is offline")
    let temp_db = TempDb::new();
    let db = &temp_db.db;

    // Create a client pointing to an offline port
    let xavier = Arc::new(
        XavierClient::new("http://127.0.0.1:54321".to_string(), "token".to_string()).unwrap(),
    );
    struct DummySynthesizer;
    #[async_trait::async_trait]
    impl gestalt_router::thinking::InsightSynthesizer for DummySynthesizer {
        async fn synthesize(&self, _executions: &[String]) -> Result<String, String> {
            Ok("insight".to_string())
        }
    }
    let loop_ = ThinkingLoop::new(xavier, Arc::new(DummySynthesizer));

    // 0 executions: should_run -> false
    assert_eq!(loop_.pending_executions_since_last_insight(db).await, 0);
    assert!(!loop_.should_run(db, MIN_EXECUTIONS).await);

    // Seed 2 executions
    db.push_event("run-1", Some("agent"), "run_started", "{}")
        .unwrap();
    db.push_event("run-1", Some("agent"), "run_finished", "{}")
        .unwrap();

    assert_eq!(loop_.pending_executions_since_last_insight(db).await, 2);
    assert!(!loop_.should_run(db, MIN_EXECUTIONS).await);

    // Seed 1 more execution (total 3)
    db.push_event("run-2", Some("agent"), "run_started", "{}")
        .unwrap();

    assert_eq!(loop_.pending_executions_since_last_insight(db).await, 3);
    assert!(loop_.should_run(db, MIN_EXECUTIONS).await);

    // Seed 2 more executions (total 5)
    db.push_event("run-2", Some("agent"), "run_finished", "{}")
        .unwrap();
    db.push_event("run-3", Some("agent"), "run_started", "{}")
        .unwrap();

    assert_eq!(loop_.pending_executions_since_last_insight(db).await, 5);
    assert!(loop_.should_run(db, MIN_EXECUTIONS).await);
}

#[tokio::test]
async fn test_should_run_past_and_future_insight_gating() {
    // describe: ThinkingLoop with mock Xavier server responses
    // it("should count executions after past insight, but ignore executions before a future insight")

    // Test 1: Last insight was in the past (e.g. 2020-01-01)
    {
        let temp_db = TempDb::new();
        let db = &temp_db.db;

        // Spawn mock Xavier server
        let (port, _server_handle) = spawn_mock_xavier("2020-01-01").await;

        let xavier = Arc::new(
            XavierClient::new(format!("http://127.0.0.1:{}", port), "token".to_string()).unwrap(),
        );
        struct DummySynthesizer;
        #[async_trait::async_trait]
        impl gestalt_router::thinking::InsightSynthesizer for DummySynthesizer {
            async fn synthesize(&self, _executions: &[String]) -> Result<String, String> {
                Ok("insight".to_string())
            }
        }
        let loop_ = ThinkingLoop::new(xavier, Arc::new(DummySynthesizer));

        // Push 3 execution events
        db.push_event("run-1", Some("agent"), "run_started", "{}")
            .unwrap();
        db.push_event("run-1", Some("agent"), "run_finished", "{}")
            .unwrap();
        db.push_event("run-2", Some("agent"), "run_started", "{}")
            .unwrap();

        // Since last insight is in 2020, today's executions are counted
        let last_time = loop_.last_insight_time().await;
        println!("Test 1 - last_insight_time: {:?}", last_time);
        let pending = loop_.pending_executions_since_last_insight(db).await;
        println!("Test 1 - pending executions: {}", pending);

        assert_eq!(pending, 3);
        assert!(loop_.should_run(db, MIN_EXECUTIONS).await);
    }

    // Test 2: Last insight is in the future (e.g. 2035-12-31)
    {
        let temp_db = TempDb::new();
        let db = &temp_db.db;

        // Spawn mock Xavier server
        let (port, _server_handle) = spawn_mock_xavier("2035-12-31").await;

        let xavier = Arc::new(
            XavierClient::new(format!("http://127.0.0.1:{}", port), "token".to_string()).unwrap(),
        );
        struct DummySynthesizer;
        #[async_trait::async_trait]
        impl gestalt_router::thinking::InsightSynthesizer for DummySynthesizer {
            async fn synthesize(&self, _executions: &[String]) -> Result<String, String> {
                Ok("insight".to_string())
            }
        }
        let loop_ = ThinkingLoop::new(xavier, Arc::new(DummySynthesizer));

        // Push 3 execution events
        db.push_event("run-1", Some("agent"), "run_started", "{}")
            .unwrap();
        db.push_event("run-1", Some("agent"), "run_finished", "{}")
            .unwrap();
        db.push_event("run-2", Some("agent"), "run_started", "{}")
            .unwrap();

        let last_time = loop_.last_insight_time().await;
        println!("Test 2 - last_insight_time: {:?}", last_time);
        let pending = loop_.pending_executions_since_last_insight(db).await;
        println!("Test 2 - pending executions: {}", pending);

        // Since last insight is in 2035, today's executions are older and should be ignored
        assert_eq!(pending, 0);
        assert!(!loop_.should_run(db, MIN_EXECUTIONS).await);
    }
}
