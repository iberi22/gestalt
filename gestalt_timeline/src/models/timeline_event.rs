//! TimelineEvent model - Core of the timeline system
//!
//! The timestamp is the PRIMARY variable of the entire system.

use super::FlexibleTimestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use surrealdb::sql::Thing;

/// Represents an event in the universal timeline.
///
/// Every action in the system is recorded as a TimelineEvent with a UTC timestamp.
/// This enables full traceability and coordination between multiple agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// Unique identifier (ULID format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,

    /// UTC timestamp - PRIMARY VARIABLE
    #[serde(with = "super::timestamp")]
    pub timestamp: FlexibleTimestamp,

    /// ID of the agent that triggered this event
    pub agent_id: String,

    /// Type of event
    pub event_type: EventType,

    /// Associated project ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,

    /// Associated task ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,

    /// Event payload data
    #[serde(default)]
    pub payload: serde_json::Value,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl TimelineEvent {
    /// Create a new timeline event with current UTC timestamp.
    pub fn new(agent_id: &str, event_type: EventType) -> Self {
        Self {
            id: None,
            timestamp: FlexibleTimestamp::now(),
            agent_id: agent_id.to_string(),
            event_type,
            project_id: None,
            task_id: None,
            payload: serde_json::Value::Null,
            metadata: HashMap::new(),
        }
    }

    /// Set the project ID for this event.
    pub fn with_project(mut self, project_id: &str) -> Self {
        self.project_id = Some(project_id.to_string());
        self
    }

    /// Set the task ID for this event.
    pub fn with_task(mut self, task_id: &str) -> Self {
        self.task_id = Some(task_id.to_string());
        self
    }

    /// Set the payload for this event.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Add metadata to this event.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Types of events that can occur in the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    /// A new project was created
    ProjectCreated,
    /// A project was updated
    ProjectUpdated,
    /// A project was deleted
    ProjectDeleted,
    /// A new task was created
    TaskCreated,
    /// A task execution started
    TaskStarted,
    /// A task completed successfully
    TaskCompleted,
    /// A task failed
    TaskFailed,
    /// A task was updated
    TaskUpdated,
    /// A task was deleted
    TaskDeleted,
    /// An agent connected to the system
    AgentConnected,
    /// An agent disconnected from the system
    AgentDisconnected,
    /// A CLI command was executed
    CommandExecuted,
    /// Sub-agent CLI launched in background
    SubAgentSpawned(String),
    /// Partial or total output of sub-agent
    SubAgentOutput(String),
    /// Sub-agent completed successfully
    SubAgentCompleted(String),
    /// Sub-agent failed
    SubAgentFailed(String),
    /// Context retrieval event
    Retrieval,
    /// Chat message from agent
    Chat,
    /// VFS Patch applied to overlay
    VfsPatchApplied,
    /// VFS Lock acquired by agent
    VfsLockAcquired,
    /// VFS Lock conflict detected
    VfsLockConflict,
    /// VFS Flush to disk started
    VfsFlushStarted,
    /// VFS Flush to disk completed
    VfsFlushCompleted,
    /// A chat message from user or agent
    ChatMessage,
    /// Custom event type
    Custom(String),
}

impl Serialize for EventType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "project_created" => Ok(EventType::ProjectCreated),
            "project_updated" => Ok(EventType::ProjectUpdated),
            "project_deleted" => Ok(EventType::ProjectDeleted),
            "task_created" => Ok(EventType::TaskCreated),
            "task_started" => Ok(EventType::TaskStarted),
            "task_completed" => Ok(EventType::TaskCompleted),
            "task_failed" => Ok(EventType::TaskFailed),
            "task_updated" => Ok(EventType::TaskUpdated),
            "task_deleted" => Ok(EventType::TaskDeleted),
            "agent_connected" => Ok(EventType::AgentConnected),
            "agent_disconnected" => Ok(EventType::AgentDisconnected),
            "command_executed" => Ok(EventType::CommandExecuted),
            "retrieval" => Ok(EventType::Retrieval),
            "chat" => Ok(EventType::Chat),
            "vfs_patch_applied" => Ok(EventType::VfsPatchApplied),
            "vfs_lock_acquired" => Ok(EventType::VfsLockAcquired),
            "vfs_lock_conflict" => Ok(EventType::VfsLockConflict),
            "vfs_flush_started" => Ok(EventType::VfsFlushStarted),
            "vfs_flush_completed" => Ok(EventType::VfsFlushCompleted),
            "chat_message" => Ok(EventType::ChatMessage),
            other => {
                if let Some(agent) = other.strip_prefix("sub_agent_spawned:") {
                    return Ok(EventType::SubAgentSpawned(agent.to_string()));
                }
                if let Some(agent) = other.strip_prefix("sub_agent_output:") {
                    return Ok(EventType::SubAgentOutput(agent.to_string()));
                }
                if let Some(agent) = other.strip_prefix("sub_agent_completed:") {
                    return Ok(EventType::SubAgentCompleted(agent.to_string()));
                }
                if let Some(agent) = other.strip_prefix("sub_agent_failed:") {
                    return Ok(EventType::SubAgentFailed(agent.to_string()));
                }
                if let Some(custom) = other.strip_prefix("custom:") {
                    Ok(EventType::Custom(custom.to_string()))
                } else {
                    Ok(EventType::Custom(other.to_string()))
                }
            }
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::ProjectCreated => write!(f, "project_created"),
            EventType::ProjectUpdated => write!(f, "project_updated"),
            EventType::ProjectDeleted => write!(f, "project_deleted"),
            EventType::TaskCreated => write!(f, "task_created"),
            EventType::TaskStarted => write!(f, "task_started"),
            EventType::TaskCompleted => write!(f, "task_completed"),
            EventType::TaskFailed => write!(f, "task_failed"),
            EventType::TaskUpdated => write!(f, "task_updated"),
            EventType::TaskDeleted => write!(f, "task_deleted"),
            EventType::AgentConnected => write!(f, "agent_connected"),
            EventType::AgentDisconnected => write!(f, "agent_disconnected"),
            EventType::CommandExecuted => write!(f, "command_executed"),
            EventType::Retrieval => write!(f, "retrieval"),
            EventType::Chat => write!(f, "chat"),
            EventType::VfsPatchApplied => write!(f, "vfs_patch_applied"),
            EventType::VfsLockAcquired => write!(f, "vfs_lock_acquired"),
            EventType::VfsLockConflict => write!(f, "vfs_lock_conflict"),
            EventType::VfsFlushStarted => write!(f, "vfs_flush_started"),
            EventType::VfsFlushCompleted => write!(f, "vfs_flush_completed"),
            EventType::ChatMessage => write!(f, "chat_message"),
            EventType::SubAgentSpawned(s) => write!(f, "sub_agent_spawned:{}", s),
            EventType::SubAgentOutput(s) => write!(f, "sub_agent_output:{}", s),
            EventType::SubAgentCompleted(s) => write!(f, "sub_agent_completed:{}", s),
            EventType::SubAgentFailed(s) => write!(f, "sub_agent_failed:{}", s),
            EventType::Custom(s) => {
                if s.starts_with("custom:") {
                    write!(f, "{}", s)
                } else {
                    write!(f, "custom:{}", s)
                }
            }
        }
    }
}
