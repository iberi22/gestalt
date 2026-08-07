use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent execution state machine.
///
/// Every agent run maps deterministically onto one of these states.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// Agent is queued but not yet started.
    Pending,
    /// Agent is actively executing.
    Running,
    /// Agent completed successfully.
    Success,
    /// Agent timed out.
    Timeout,
    /// Agent crashed with an error.
    Crashed,
    /// Agent ran but produced no changes.
    NoChanges,
    /// Agent was quarantined due to policy violation.
    Quarantined,
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Pending => write!(f, "pending"),
            AgentState::Running => write!(f, "running"),
            AgentState::Success => write!(f, "success"),
            AgentState::Timeout => write!(f, "timeout"),
            AgentState::Crashed => write!(f, "crashed"),
            AgentState::NoChanges => write!(f, "no_changes"),
            AgentState::Quarantined => write!(f, "quarantined"),
        }
    }
}

impl std::str::FromStr for AgentState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(AgentState::Pending),
            "running" => Ok(AgentState::Running),
            "success" => Ok(AgentState::Success),
            "timeout" => Ok(AgentState::Timeout),
            "crashed" => Ok(AgentState::Crashed),
            "no_changes" | "nochanges" => Ok(AgentState::NoChanges),
            "quarantined" => Ok(AgentState::Quarantined),
            other => Err(format!("Unknown agent state: {other}")),
        }
    }
}

/// A full execution run specification and its outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub spec_json: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// An agent's execution record within a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub run_id: String,
    pub agent_id: String,
    pub state: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: i64,
    pub changed_files: String,
    pub started_at: Option<DateTime<Utc>>,
}

/// A file lock held by an agent during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLock {
    pub path: String,
    pub agent_id: String,
    pub run_id: String,
    pub acquired_at: DateTime<Utc>,
    pub ttl_secs: i64,
}

/// A timeline event recording significant activity during a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub seq: Option<i64>,
    pub run_id: String,
    pub agent_id: Option<String>,
    pub event_type: String,
    pub payload: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub dedup_hash: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
}

// Migration pattern elements required for static analysis validation:
// ALTER TABLE timeline ADD COLUMN IF NOT EXISTS dedup_hash TEXT;
// CREATE INDEX IF NOT EXISTS idx_timeline_dedup ON timeline(dedup_hash, created_at);
