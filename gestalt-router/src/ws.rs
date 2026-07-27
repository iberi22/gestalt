//! WebSocket bridge for the gestalt-router.
//!
//! [`WsRouterBridge`] connects the Router's state changes to a
//! [`gestalt_ws::WsServer`], forwarding timeline events to all
//! connected WebSocket clients.

use gestalt_state::memstate::MemState;
use gestalt_state::TimelineEvent;
use gestalt_ws::WsEvent;
use gestalt_ws::WsServer;
/// Bridges Router state changes to a WebSocket server.
///
/// Subscribes to [`MemState`] state-change broadcasts and forwards
/// them as [`WsEvent`] messages to all connected WebSocket clients.
/// Also provides helper methods to emit run-level events from the
/// Router's `execute()` method.
#[derive(Clone)]
pub struct WsRouterBridge {
    /// The WebSocket server to broadcast through.
    ws_server: WsServer,
}

impl WsRouterBridge {
    /// Create a new bridge wrapping a `WsServer`.
    pub fn new(ws_server: WsServer) -> Self {
        Self { ws_server }
    }

    /// Get a reference to the inner WsServer.
    pub fn ws_server(&self) -> &WsServer {
        &self.ws_server
    }

    /// Subscribe to MemState and forward all state-change events
    /// to the WebSocket broadcast channel.
    ///
    /// Spawns a background task that reads from `mem_state.subscribe()`
    /// and forwards each [`TimelineEvent`] as a [`WsEvent::StateChanged`]
    /// to the WebSocket server.
    pub fn subscribe_to_memstate(&self, mem_state: &MemState) {
        let ws_server = self.ws_server.clone();
        let mut rx = mem_state.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(timeline_event) => {
                        Self::forward_timeline_event(&ws_server, &timeline_event).await;
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WsRouterBridge lagged by {n} MemState events");
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!("WsRouterBridge: MemState broadcast closed");
                        break;
                    },
                }
            }
        });
    }

    /// Forward a `TimelineEvent` from MemState as a `WsEvent` broadcast.
    async fn forward_timeline_event(ws_server: &WsServer, event: &TimelineEvent) {
        // Map the generic TimelineEvent to a structured WsEvent
        let agent_id = event.agent_id.as_deref().unwrap_or("");
        let ws_event = match event.event_type.as_str() {
            "state_change" => {
                // Extract state from payload
                let state = serde_json::from_str::<serde_json::Value>(&event.payload)
                    .ok()
                    .and_then(|v| v.get("state").and_then(|s| s.as_str().map(String::from)))
                    .unwrap_or_else(|| "unknown".to_string());

                WsEvent::StateChanged {
                    run_id: event.run_id.clone(),
                    agent_id: agent_id.to_string(),
                    state,
                }
            },
            "lock_acquired" => {
                let path = serde_json::from_str::<serde_json::Value>(&event.payload)
                    .ok()
                    .and_then(|v| v.get("path").and_then(|s| s.as_str().map(String::from)))
                    .unwrap_or_else(|| "unknown".to_string());

                WsEvent::LockAcquired {
                    run_id: event.run_id.clone(),
                    agent_id: agent_id.to_string(),
                    path,
                }
            },
            "lock_released" => {
                let path = serde_json::from_str::<serde_json::Value>(&event.payload)
                    .ok()
                    .and_then(|v| v.get("path").and_then(|s| s.as_str().map(String::from)))
                    .unwrap_or_else(|| "unknown".to_string());

                WsEvent::LockReleased {
                    run_id: event.run_id.clone(),
                    agent_id: agent_id.to_string(),
                    path,
                }
            },
            "conflict_detected" => {
                let payload = serde_json::from_str::<serde_json::Value>(&event.payload).ok();

                let path = payload
                    .as_ref()
                    .and_then(|v| v.get("path"))
                    .and_then(|v| {
                        if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "unknown".to_string());

                let agent_a = payload
                    .as_ref()
                    .and_then(|v| v.get("agent_a"))
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "unknown".to_string());

                let agent_b = payload
                    .as_ref()
                    .and_then(|v| v.get("agent_b"))
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "unknown".to_string());

                let message = payload
                    .as_ref()
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "lock conflict detected".to_string());

                WsEvent::ConflictDetected {
                    run_id: event.run_id.clone(),
                    agent_a,
                    agent_b,
                    path,
                    message,
                }
            },
            _ => {
                // For unrecognised event types, skip silently
                return;
            },
        };

        ws_server.broadcast(&ws_event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gestalt_state::TimelineEvent;

    #[tokio::test]
    async fn test_forward_state_change_event() {
        let (server, mut rx) = WsServer::new();
        let event = TimelineEvent {
            seq: None,
            run_id: "test-run".into(),
            agent_id: Some("agent-1".into()),
            event_type: "state_change".into(),
            payload: r#"{"state":"running"}"#.into(),
            created_at: chrono::Utc::now(),
        };

        WsRouterBridge::forward_timeline_event(&server, &event).await;

        let received: String = rx.recv().await.unwrap();
        let ws_event: WsEvent = serde_json::from_str(&received).unwrap();
        assert_eq!(
            ws_event,
            WsEvent::StateChanged {
                run_id: "test-run".into(),
                agent_id: "agent-1".into(),
                state: "running".into(),
            }
        );
    }

    #[tokio::test]
    async fn test_forward_lock_acquired_event() {
        let (server, mut rx) = WsServer::new();
        let event = TimelineEvent {
            seq: None,
            run_id: "test-run".into(),
            agent_id: Some("agent-1".into()),
            event_type: "lock_acquired".into(),
            payload: r#"{"path":"/tmp/test.lock"}"#.into(),
            created_at: chrono::Utc::now(),
        };

        WsRouterBridge::forward_timeline_event(&server, &event).await;

        let received: String = rx.recv().await.unwrap();
        let ws_event: WsEvent = serde_json::from_str(&received).unwrap();
        assert_eq!(
            ws_event,
            WsEvent::LockAcquired {
                run_id: "test-run".into(),
                agent_id: "agent-1".into(),
                path: "/tmp/test.lock".into(),
            }
        );
    }

    #[tokio::test]
    async fn test_skip_unknown_event_type() {
        let (server, _rx) = WsServer::new();
        let event = TimelineEvent {
            seq: None,
            run_id: "test-run".into(),
            agent_id: None,
            event_type: "some_unknown_type".into(),
            payload: "{}".into(),
            created_at: chrono::Utc::now(),
        };

        // This should not panic or broadcast anything
        WsRouterBridge::forward_timeline_event(&server, &event).await;
        // If it reaches here, the test passes (unknown types are silently skipped)
    }
}
