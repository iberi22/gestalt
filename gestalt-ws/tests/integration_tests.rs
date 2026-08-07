//! Integration tests for gestalt-ws.
//!
//! Verifies WebSocket publishing, subscription, and filtering under the unified EventStream interface.

use futures_util::StreamExt;
use gestalt_ws::{publish_to_all_adapters, BusEvent, EventStream, WsServer};
use std::net::SocketAddr;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Find a free ephemeral TCP port on localhost for test isolation.
fn free_addr() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

/// Helper to connect a WebSocket client to the local test server.
async fn connect_ws(
    addr: SocketAddr,
) -> impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin {
    let (ws, _) = connect_async(format!("ws://{addr}")).await.unwrap();
    ws
}

/// Helper to read a Text message with a timeout.
async fn recv_text(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> String {
    match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(other))) => panic!("Expected Text frame, got: {other:?}"),
        Ok(Some(Err(e))) => panic!("WebSocket error: {e}"),
        Ok(None) => panic!("WebSocket stream ended unexpectedly"),
        Err(_) => panic!("Timeout (5s) waiting for WebSocket message"),
    }
}

/// Mock EventStream implementation for testing registration/publishing.
#[derive(Clone)]
struct MockEventStream {
    server: WsServer,
}

impl EventStream for MockEventStream {
    fn publish(&self, ev: &BusEvent) {
        self.server.broadcast_bus(ev);
    }

    fn subscribe(&self, filter: Option<String>) -> gestalt_ws::BoxStream<'static, BusEvent> {
        let rx = self.server.broadcast_bus_tx.subscribe();
        let s = futures_util::stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => return Some((ev, rx)),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        });

        if let Some(f) = filter {
            s.filter(move |ev| {
                let f_clone = f.clone();
                let ev_type = ev.event_type.clone();
                let ev_agent = ev.agent.clone();
                async move { ev_type == f_clone || ev_agent == f_clone }
            })
            .boxed()
        } else {
            s.boxed()
        }
    }
}

#[tokio::test]
async fn test_ws_publish_receive_bus_event() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;
    let mut client = connect_ws(addr).await;

    // Register our mock event stream (which wraps WsServer) in ACTIVE_ADAPTERS
    let adapter = std::sync::Arc::new(MockEventStream {
        server: server.clone(),
    });
    gestalt_ws::register_adapter(adapter);

    let event = BusEvent {
        agent: "test-agent".to_string(),
        event_type: "run_started".to_string(),
        run_id: Some("run-abc".to_string()),
        project: Some("gestalt".to_string()),
        state: Some("Running".to_string()),
        summary: "Pipeline started".to_string(),
        metadata: serde_json::json!({"triggered_by": "test"}),
        ts: "2026-08-05T12:00:00Z".to_string(),
    };

    // Publish using the decoupled global registry
    publish_to_all_adapters(&event);

    let text = recv_text(&mut client).await;
    let received: BusEvent = serde_json::from_str(&text).unwrap();

    assert_eq!(received.agent, "test-agent");
    assert_eq!(received.event_type, "run_started");
    assert_eq!(received.run_id, Some("run-abc".to_string()));
    assert_eq!(received.summary, "Pipeline started");
}

#[tokio::test]
async fn test_event_stream_subscribe_filtering() {
    let (server, _rx) = WsServer::new();
    let adapter = MockEventStream { server };

    // Subscribe to only "checkpoint" events
    let mut stream = adapter.subscribe(Some("checkpoint".to_string()));

    let event1 = BusEvent {
        agent: "jules".to_string(),
        event_type: "run_started".to_string(),
        run_id: None,
        project: None,
        state: None,
        summary: "Ignored".to_string(),
        metadata: serde_json::Value::Null,
        ts: String::new(),
    };

    let event2 = BusEvent {
        agent: "jules".to_string(),
        event_type: "checkpoint".to_string(),
        run_id: None,
        project: None,
        state: None,
        summary: "Matched".to_string(),
        metadata: serde_json::Value::Null,
        ts: String::new(),
    };

    // Publish both events
    adapter.publish(&event1);
    adapter.publish(&event2);

    // The stream should receive the matching one first
    let received = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(received.event_type, "checkpoint");
    assert_eq!(received.summary, "Matched");
}
