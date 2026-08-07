//! WebSocket bridge for the gestalt-router.
//!
//! [`WsRouterBridge`] connects the Router's state changes to a
//! [`gestalt_ws::WsServer`], forwarding timeline events to all
//! connected WebSocket clients.

pub use gestalt_ws;

use gestalt_state::memstate::MemState;
use gestalt_state::TimelineEvent;
use gestalt_ws::WsEvent;
use gestalt_ws::WsServer;
use std::sync::Arc;

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
        let bridge = Self { ws_server };
        gestalt_ws::register_adapter(Arc::new(bridge.clone()));
        bridge
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
                    .and_then(|v| v.as_str().map(str::to_string))
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

        let redacted_event = crate::xavier_sink::redact_ws_event(ws_event);
        ws_server.broadcast(&redacted_event).await;
    }
}

impl gestalt_ws::EventStream for WsRouterBridge {
    fn publish(&self, ev: &gestalt_ws::BusEvent) {
        self.ws_server.broadcast_bus(ev);
    }

    fn subscribe(&self, filter: Option<String>) -> gestalt_ws::BoxStream<'static, gestalt_ws::BusEvent> {
        use gestalt_ws::StreamExt;
        let rx = self.ws_server.broadcast_bus_tx.subscribe();

        let s = gestalt_ws::futures_util::stream::unfold(rx, |mut rx| async move {
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
                async move {
                    ev_type == f_clone || ev_agent == f_clone
                }
            }).boxed()
        } else {
            s.boxed()
        }
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
            dedup_hash: None,
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
            dedup_hash: None,
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
            dedup_hash: None,
        };

        // This should not panic or broadcast anything
        WsRouterBridge::forward_timeline_event(&server, &event).await;
        // If it reaches here, the test passes (unknown types are silently skipped)
    }
}
