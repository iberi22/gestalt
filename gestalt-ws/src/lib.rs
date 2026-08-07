//! Gestalt WebSocket server — broadcasts timeline events to connected clients.
//!
//! # Architecture
//!
//! ```text
//!   MemState (broadcast::Sender<TimelineEvent>)
//!        │
//!        ▼
//!   WsServer::start()       ◄── tokio::spawn in Router
//!        │
//!        ├── accept loop (TcpListener on port :3001)
//!        │       │
//!        │       └── per‑connection task: subscribe → convert → forward JSON
//!        │
//!        └── shutdown via watch::Sender<bool>
//! ```
//!
//! The server subscribes to [`MemState`]'s broadcast channel and maps
//! recognised [`TimelineEvent`] variants to [`WsEvent`] values:
//!
//! | MemState `event_type` | WsEvent variant       |
//! |------------------------|-----------------------|
//! | `"state_change"`       | `StateChanged`        |
//! | `"lock_acquired"`      | `LockAcquired`        |
//! | `"lock_released"`      | `LockReleased`        |

use std::net::SocketAddr;
use std::sync::Arc;
pub use futures_util::{SinkExt, StreamExt};
pub use futures_util::stream::BoxStream;
pub use futures_util;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

/// Current protocol schema version for WebSocket events.
pub const CURRENT_VERSION: u32 = 1;

fn default_version() -> u32 {
    CURRENT_VERSION
}

/// Envelope wrapping standard WebSocket events with schema versioning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WsEnvelope {
    /// The protocol schema version of the event.
    #[serde(default = "default_version")]
    pub version: u32,
    /// The flattened WebSocket event.
    #[serde(flatten)]
    pub event: WsEvent,
}

/// Events that can be broadcast to WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WsEvent {
    /// An agent's state has changed.
    StateChanged {
        run_id: String,
        agent_id: String,
        state: String,
    },
    /// A file lock was acquired by an agent.
    LockAcquired {
        run_id: String,
        agent_id: String,
        path: String,
    },
    /// A file lock was released by an agent.
    LockReleased {
        run_id: String,
        agent_id: String,
        path: String,
    },
    /// A run has started.
    RunStarted {
        run_id: String,
        task: String,
        agents: Vec<String>,
    },
    /// A run has finished.
    RunFinished { run_id: String, summary: String },
    /// A real-time lock conflict was detected between two agents.
    ConflictDetected {
        run_id: String,
        agent_a: String,
        agent_b: String,
        path: String,
        message: String,
    },
}

impl WsEvent {
    /// Serialize this event to a JSON string for WebSocket broadcast.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// A structured event pushed onto the universal information bus.
///
/// Carries full traceability metadata: who sent it (`agent`), what happened
/// (`event_type`, `state`), which run/project it belongs to, and free-form
/// `metadata` for LLM/provider/decision context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusEvent {
    /// Originating agent: "hermes" | "jules" | "agent-cli" | "gestalt" | ...
    pub agent: String,
    /// Event kind: "run_started" | "agent_state" | "checkpoint" | "decision" | ...
    pub event_type: String,
    /// Run identifier, when the event belongs to a Gestalt orchestration run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Project name (e.g. "gestalt", "xavier", "nido").
    #[serde(default)]
    pub project: Option<String>,
    /// Agent state: Pending | Running | Success | Timeout | Crashed | NoChanges.
    #[serde(default)]
    pub state: Option<String>,
    /// Human-readable one-line summary of what happened.
    pub summary: String,
    /// Free-form traceability metadata: {"llm": "...", "provider": "...",
    /// "decision": "...", "requested_by": "...", "tool_calls": N, ...}.
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// RFC3339 timestamp of the event.
    #[serde(default)]
    pub ts: String,
}

/// Decoupled EventStream trait to support unified adapter behaviors.
pub trait EventStream {
    /// Publish a BusEvent onto the WebSocket stream.
    fn publish(&self, ev: &BusEvent);
    /// Subscribe to the WebSocket stream, optionally filtered by event_type or agent.
    fn subscribe(&self, filter: Option<String>) -> BoxStream<'static, BusEvent>;
}

static ACTIVE_ADAPTERS: std::sync::OnceLock<std::sync::RwLock<Vec<Arc<dyn EventStream + Send + Sync>>>> = std::sync::OnceLock::new();

/// Register an active WebSocket adapter instance.
pub fn register_adapter(adapter: Arc<dyn EventStream + Send + Sync>) {
    let adapters = ACTIVE_ADAPTERS.get_or_init(|| std::sync::RwLock::new(Vec::new()));
    if let Ok(mut lock) = adapters.write() {
        lock.push(adapter);
    }
}

/// Publish an event to all registered WebSocket adapters.
pub fn publish_to_all_adapters(ev: &BusEvent) {
    if let Some(adapters) = ACTIVE_ADAPTERS.get() {
        if let Ok(lock) = adapters.read() {
            for adapter in lock.iter() {
                adapter.publish(ev);
            }
        }
    }
}

/// A lightweight WebSocket server that broadcasts timeline events
/// to all connected clients.
///
/// The server listens on a configured address, accepts WebSocket
/// connections, and fans out every [`broadcast`](Self::broadcast) call
/// to every connected client as a JSON text frame.
#[derive(Clone)]
pub struct WsServer {
    /// Broadcast sender — cloned for each connected client on subscribe.
    pub broadcast_tx: broadcast::Sender<String>,
    /// Dedicated broadcast channel for BusEvents (stream subscribe).
    pub broadcast_bus_tx: broadcast::Sender<BusEvent>,
    /// Handle to the TCP accept loop (dropped on shutdown).
    #[allow(dead_code)]
    accept_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
}

impl WsServer {
    /// Create a new `WsServer` without binding to a port.
    ///
    /// Returns the server instance and a receiver for the broadcast
    /// channel (useful for testing).
    pub fn new() -> (Self, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(1024);
        let (bus_tx, _) = broadcast::channel(1024);
        let server = Self {
            broadcast_tx: tx,
            broadcast_bus_tx: bus_tx,
            accept_handle: Arc::new(tokio::sync::Mutex::new(None)),
        };
        (server, rx)
    }

    /// Start the WebSocket server on `addr`.
    ///
    /// Spawns a background task that accepts incoming TCP connections
    /// and upgrades them to WebSocket. Each connected client receives
    /// all future [`broadcast`](Self::broadcast) events.
    pub async fn bind(addr: SocketAddr) -> Self {
        let (tx, _) = broadcast::channel(1024);
        let (bus_tx, _) = broadcast::channel(1024);
        let listener = TcpListener::bind(addr).await.unwrap_or_else(|e| {
            panic!("Failed to bind WsServer to {addr}: {e}");
        });

        info!("WebSocket server listening on {addr}");

        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            Self::accept_loop(listener, tx_clone).await;
        });

        Self {
            broadcast_tx: tx,
            broadcast_bus_tx: bus_tx,
            accept_handle: Arc::new(tokio::sync::Mutex::new(Some(handle))),
        }
    }

    /// Broadcast a `WsEvent` to all connected WebSocket clients.
    ///
    /// The event is wrapped in a `WsEnvelope` with the current schema version,
    /// serialised to JSON, and sent to every client.
    /// If a client has disconnected, its receiver is silently dropped.
    /// This is fire-and-forget — errors are logged but not returned.
    pub async fn broadcast(&self, event: &WsEvent) {
        let envelope = WsEnvelope {
            version: CURRENT_VERSION,
            event: event.clone(),
        };

        match serde_json::to_string(&envelope) {
            Ok(json) => {
                let count = self.broadcast_tx.send(json);
                match count {
                    Ok(n) => {
                        debug!("Broadcast WsEvent (wrapped in WsEnvelope) to {n} subscribers");
                    },
                    Err(_) => {
                        // No receivers — normal when no clients are connected
                        debug!("WsEnvelope broadcast: no receivers");
                    },
                }
            },
            Err(e) => {
                error!("Failed to serialise WsEnvelope for broadcast: {e}");
            },
        }
    }

    /// Broadcast a canonical `BusEvent` to all connected WebSocket clients.
    ///
    /// The event is serialised to raw JSON (without WsEnvelope wrapping, to maintain parity with HTTP),
    /// and sent to every client. It is also dispatched to subscribers of the EventStream.
    pub fn broadcast_bus(&self, event: &BusEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            let _ = self.broadcast_tx.send(json);
        }
        let _ = self.broadcast_bus_tx.send(event.clone());
    }

    /// Get the number of currently connected clients.
    pub fn connected_clients(&self) -> usize {
        self.broadcast_tx.receiver_count()
    }

    /// Accept loop — runs forever accepting connections.
    async fn accept_loop(listener: TcpListener, tx: broadcast::Sender<String>) {
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    debug!("New WebSocket connection from {peer_addr}");
                    let peer_rx = tx.subscribe();
                    tokio::spawn(async move {
                        Self::handle_connection(stream, peer_addr, peer_rx).await;
                    });
                },
                Err(e) => {
                    error!("Failed to accept connection: {e}");
                    // Brief pause to avoid busy-loop on persistent errors
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                },
            }
        }
    }

    /// Handle a single WebSocket connection.
    ///
    /// Upgrades the TCP stream to WebSocket, then forwards every
    /// broadcast event to the client until the client disconnects.
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
        mut rx: broadcast::Receiver<String>,
    ) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                warn!("WebSocket handshake failed for {peer_addr}: {e}");
                return;
            },
        };

        let (mut write, mut read) = ws_stream.split();

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(json) => {
                            if let Err(e) = write.send(Message::Text(json)).await {
                                debug!("WebSocket send failed for {peer_addr}: {e}");
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("WebSocket client {peer_addr} lagged by {n} messages");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            debug!("WebSocket broadcast channel closed for {peer_addr}");
                            break;
                        }
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Close(_))) | None => {
                            debug!("WebSocket client {peer_addr} disconnected");
                            break;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_event_serialization_roundtrip() {
        let events = vec![
            WsEvent::StateChanged {
                run_id: "run-1".into(),
                agent_id: "agent-1".into(),
                state: "running".into(),
            },
            WsEvent::LockAcquired {
                run_id: "run-1".into(),
                agent_id: "agent-1".into(),
                path: "/tmp/test.lock".into(),
            },
            WsEvent::LockReleased {
                run_id: "run-1".into(),
                agent_id: "agent-1".into(),
                path: "/tmp/test.lock".into(),
            },
            WsEvent::RunStarted {
                run_id: "run-1".into(),
                task: "test task".into(),
                agents: vec!["agent-1".into(), "agent-2".into()],
            },
            WsEvent::RunFinished {
                run_id: "run-1".into(),
                summary: "completed with 2 agents".into(),
            },
            WsEvent::ConflictDetected {
                run_id: "run-1".into(),
                agent_a: "agent-1".into(),
                agent_b: "agent-2".into(),
                path: "src/file.rs".into(),
                message: "Conflict: agents agent-1 and agent-2 both locked src/file.rs".into(),
            },
        ];

        for event in &events {
            let json = event.to_json().unwrap();
            let deser: WsEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(*event, deser, "roundtrip failed for event: {:?}", event);
        }
    }

    #[test]
    fn test_ws_event_json_format_tagged() {
        let event = WsEvent::StateChanged {
            run_id: "r1".into(),
            agent_id: "a1".into(),
            state: "running".into(),
        };
        let json = event.to_json().unwrap();
        assert!(json.contains("\"run_id\""));
        assert!(json.contains("\"agent_id\""));
        assert!(json.contains("\"state\""));
    }

    #[test]
    fn test_ws_envelope_roundtrip() {
        let event = WsEvent::StateChanged {
            run_id: "r1".into(),
            agent_id: "a1".into(),
            state: "running".into(),
        };
        let envelope = WsEnvelope {
            version: CURRENT_VERSION,
            event: event.clone(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        // Check that version field is present at root level
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"type\":\"state_changed\""));

        let deser: WsEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.version, CURRENT_VERSION);
        assert_eq!(deser.event, event);
    }

    #[test]
    fn test_ws_envelope_backward_compatibility() {
        // Without version field, should default to CURRENT_VERSION
        let raw_json =
            r#"{"type":"state_changed","data":{"run_id":"r1","agent_id":"a1","state":"running"}}"#;
        let deser: WsEnvelope = serde_json::from_str(raw_json).unwrap();
        assert_eq!(deser.version, CURRENT_VERSION);
        if let WsEvent::StateChanged {
            run_id,
            agent_id,
            state,
        } = deser.event
        {
            assert_eq!(run_id, "r1");
            assert_eq!(agent_id, "a1");
            assert_eq!(state, "running");
        } else {
            panic!("Expected StateChanged variant");
        }
    }

    #[tokio::test]
    async fn test_ws_server_new_broadcasts() {
        let (server, mut rx) = WsServer::new();

        let event = WsEvent::StateChanged {
            run_id: "test-run".into(),
            agent_id: "agent-1".into(),
            state: "running".into(),
        };

        server.broadcast(&event).await;

        let received: String = rx.recv().await.unwrap();
        let deser: WsEnvelope = serde_json::from_str(&received).unwrap();
        assert_eq!(deser.event, event);
        assert_eq!(deser.version, CURRENT_VERSION);
    }

    #[tokio::test]
    async fn test_ws_server_connected_clients() {
        let (server, _rx) = WsServer::new();
        // _rx from new() counts as one subscriber
        assert_eq!(server.connected_clients(), 1);

        let _rx2 = server.broadcast_tx.subscribe();
        assert_eq!(server.connected_clients(), 2);
    }

    #[tokio::test]
    async fn test_ws_server_multiple_subscribers() {
        let (server, _rx1) = WsServer::new();
        let mut rx2 = server.broadcast_tx.subscribe();
        let mut rx3 = server.broadcast_tx.subscribe();

        let event = WsEvent::RunStarted {
            run_id: "multi-test".into(),
            task: "multi-subscriber task".into(),
            agents: vec!["agent-a".into()],
        };

        server.broadcast(&event).await;

        // Both subscribers should receive the event
        let received2: String = rx2.recv().await.unwrap();
        let received3: String = rx3.recv().await.unwrap();
        let deser2: WsEnvelope = serde_json::from_str(&received2).unwrap();
        let deser3: WsEnvelope = serde_json::from_str(&received3).unwrap();
        assert_eq!(deser2.event, event);
        assert_eq!(deser3.event, event);
    }
}
