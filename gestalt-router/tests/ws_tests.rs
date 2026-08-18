//! WebSocket integration tests for gestalt-ws + gestalt-router.
//!
//! These tests spin up a real [`WsServer`] TCP listener and connect
//! WebSocket clients via `tokio-tungstenite` to verify the full
//! broadcast pipeline end-to-end.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::StreamExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use gestalt_router::ws::WsRouterBridge;
use gestalt_state::MemState;
use gestalt_ws::{WsEvent, WsServer};

// ── Helpers ──────────────────────────────────────────────────────────────

/// Find a free TCP port on localhost by asking the kernel for an ephemeral
/// port, then releasing it.  There is a tiny race window, but in practice
/// it works reliably for test scenarios.
fn free_addr() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

/// Helper: connect a WebSocket client and return the stream.
async fn connect_ws(
    addr: SocketAddr,
) -> impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin {
    let (ws, _) = connect_async(format!("ws://{addr}")).await.unwrap();
    ws
}

/// Helper: read the next text message from a WebSocket stream with a
/// 5-second timeout.  Panics on unexpected message types, errors, or
/// timeouts.
async fn recv_text(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> String {
    match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => text.to_string(),
        Ok(Some(Ok(other))) => panic!("Expected Text frame, got: {other:?}"),
        Ok(Some(Err(e))) => panic!("WebSocket error: {e}"),
        Ok(None) => panic!("WebSocket stream ended unexpectedly"),
        Err(_) => panic!("Timeout (5s) waiting for WebSocket message"),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

/// Test 1: Start WsServer, connect one client, broadcast an event, verify
/// the client receives it with the correct payload.
#[tokio::test]
async fn test_ws_connect_and_receive() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;
    let mut client = connect_ws(addr).await;

    let event = WsEvent::StateChanged {
        run_id: "run-1".into(),
        agent_id: "agent-1".into(),
        state: "running".into(),
    };

    server.broadcast(&event).await;

    let text = recv_text(&mut client).await;
    let received: WsEvent = serde_json::from_str(&text).unwrap();
    assert_eq!(received, event);
}

/// Test 2: Multiple clients (5) all receive the same broadcast.
#[tokio::test]
async fn test_ws_broadcast_multiple_clients() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;

    // Connect 5 clients sequentially
    let mut clients = Vec::new();
    for _ in 0..5 {
        clients.push(connect_ws(addr).await);
    }

    let event = WsEvent::StateChanged {
        run_id: "run-5".into(),
        agent_id: "agent-all".into(),
        state: "broadcast".into(),
    };

    server.broadcast(&event).await;

    // All 5 clients should receive the event
    for client in clients.iter_mut() {
        let text = recv_text(client).await;
        let received: WsEvent = serde_json::from_str(&text).unwrap();
        assert_eq!(
            received, event,
            "All clients should receive identical event"
        );
    }
}

/// Test 3: Verify WsEvent::StateChanged payload fields are correctly
/// transmitted through the WebSocket.
#[tokio::test]
async fn test_ws_state_changed_payload() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;
    let mut client = connect_ws(addr).await;

    let event = WsEvent::StateChanged {
        run_id: "integration-test".into(),
        agent_id: "test-agent-42".into(),
        state: "failed".into(),
    };

    server.broadcast(&event).await;

    let text = recv_text(&mut client).await;
    let received: WsEvent = serde_json::from_str(&text).unwrap();

    // Verify specific fields
    match received {
        WsEvent::StateChanged {
            run_id,
            agent_id,
            state,
        } => {
            assert_eq!(run_id, "integration-test");
            assert_eq!(agent_id, "test-agent-42");
            assert_eq!(state, "failed");
        },
        other => panic!("Expected StateChanged, got: {other:?}"),
    }
}

/// Test 4: Client disconnect + reconnect.  After a client drops, a new
/// connection still receives subsequent broadcasts.
#[tokio::test]
async fn test_ws_disconnect_reconnect() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;

    // ── Connect first client ──
    let mut client1 = connect_ws(addr).await;

    let event1 = WsEvent::StateChanged {
        run_id: "session-1".into(),
        agent_id: "alpha".into(),
        state: "started".into(),
    };
    server.broadcast(&event1).await;

    let text = recv_text(&mut client1).await;
    let received: WsEvent = serde_json::from_str(&text).unwrap();
    assert_eq!(received, event1);

    // ── Disconnect client1 ──
    drop(client1);

    // Brief pause so the server notices the disconnect
    tokio::time::sleep(Duration::from_millis(200)).await;

    // ── Reconnect as client2 ──
    let mut client2 = connect_ws(addr).await;

    let event2 = WsEvent::StateChanged {
        run_id: "session-2".into(),
        agent_id: "beta".into(),
        state: "running".into(),
    };
    server.broadcast(&event2).await;

    let text = recv_text(&mut client2).await;
    let received: WsEvent = serde_json::from_str(&text).unwrap();
    assert_eq!(received, event2);

    // The old client1 should NOT have received event2 (it was dropped).
    // Since we dropped it, this is guaranteed.
}

/// Test 5: Stress test — 100 events in succession.
#[tokio::test]
async fn test_ws_stress_100_events() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;
    let mut client = connect_ws(addr).await;

    let count = 100;

    for i in 0..count {
        let event = WsEvent::StateChanged {
            run_id: "stress-test".into(),
            agent_id: format!("agent-{i}"),
            state: format!("state-{i}"),
        };
        server.broadcast(&event).await;
    }

    // Receive all 100 events
    for i in 0..count {
        let text = recv_text(&mut client).await;
        let received: WsEvent = serde_json::from_str(&text).unwrap();
        match received {
            WsEvent::StateChanged {
                run_id,
                agent_id,
                state,
            } => {
                assert_eq!(run_id, "stress-test");
                assert_eq!(agent_id, format!("agent-{i}"));
                assert_eq!(state, format!("state-{i}"));
            },
            other => panic!("Expected StateChanged, got: {other:?}"),
        }
    }
}

/// Bonus: Full integration test exercising the MemState →
/// WsRouterBridge → WsServer → WebSocket client pipeline.
#[tokio::test]
async fn test_ws_full_pipeline_via_memstate() {
    let addr = free_addr();
    let server = WsServer::bind(addr).await;
    let mut client = connect_ws(addr).await;

    // Create MemState and bridge it to the WebSocket server
    let mem = MemState::new();
    let bridge = WsRouterBridge::new(server.clone());
    bridge.subscribe_to_memstate(&mem);

    // Trigger a state change through MemState — this should flow through
    // the bridge and appear on the WebSocket.
    mem.set_agent_state("full-pipeline-run", "agent-omega", "running");

    let text = recv_text(&mut client).await;
    let received: WsEvent = serde_json::from_str(&text).unwrap();
    match received {
        WsEvent::StateChanged {
            run_id,
            agent_id,
            state,
        } => {
            assert_eq!(run_id, "full-pipeline-run");
            assert_eq!(agent_id, "agent-omega");
            assert_eq!(state, "running");
        },
        other => panic!("Expected StateChanged, got: {other:?}"),
    }
}
