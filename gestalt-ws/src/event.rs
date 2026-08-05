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
}
