use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use crate::event::WsEvent;

/// A lightweight WebSocket server that broadcasts timeline events
/// to all connected clients.
///
/// The server listens on a configured address, accepts WebSocket
/// connections, and fans out every [`broadcast`](Self::broadcast) call
/// to every connected client as a JSON text frame.
#[derive(Clone)]
pub struct WsServer {
    /// Broadcast sender — cloned for each connected client on subscribe.
    broadcast_tx: broadcast::Sender<String>,
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
        let server = Self {
            broadcast_tx: tx,
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
            accept_handle: Arc::new(tokio::sync::Mutex::new(Some(handle))),
        }
    }

    /// Broadcast a `WsEvent` to all connected WebSocket clients.
    ///
    /// The event is serialised to JSON and sent to every client.
    /// If a client has disconnected, its receiver is silently dropped.
    /// This is fire-and-forget — errors are logged but not returned.
    pub async fn broadcast(&self, event: &WsEvent) {
        match event.to_json() {
            Ok(json) => {
                let count = self.broadcast_tx.send(json);
                match count {
                    Ok(n) => {
                        debug!("Broadcast WsEvent to {n} subscribers");
                    }
                    Err(_) => {
                        // No receivers — normal when no clients are connected
                        debug!("WsEvent broadcast: no receivers");
                    }
                }
            }
            Err(e) => {
                error!("Failed to serialise WsEvent for broadcast: {e}");
            }
        }
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
                }
                Err(e) => {
                    error!("Failed to accept connection: {e}");
                    // Brief pause to avoid busy-loop on persistent errors
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
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
            }
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
        let deser: WsEvent = serde_json::from_str(&received).unwrap();
        assert_eq!(deser, event);
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
        let deser2: WsEvent = serde_json::from_str(&received2).unwrap();
        let deser3: WsEvent = serde_json::from_str(&received3).unwrap();
        assert_eq!(deser2, event);
        assert_eq!(deser3, event);
    }
}
