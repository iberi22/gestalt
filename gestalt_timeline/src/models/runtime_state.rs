//! Persistent runtime state for autonomous agent loops.

use super::FlexibleTimestamp;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

/// Persisted state snapshot for a running autonomous agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub agent_id: String,
    pub goal: String,
    pub phase: RuntimePhase,
    pub current_step: usize,
    /// `0` means elastic (no fixed limit); otherwise hard safety cap.
    pub max_steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_observation: Option<String>,
    pub history_tail: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(with = "crate::models::timestamp")]
    pub started_at: FlexibleTimestamp,
    #[serde(with = "crate::models::timestamp")]
    pub updated_at: FlexibleTimestamp,
    #[serde(with = "crate::models::timestamp::option")]
    pub finished_at: Option<FlexibleTimestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePhase {
    Running,
    Completed,
    Failed,
}

impl AgentRuntimeState {
    pub fn new(agent_id: &str, goal: &str, max_steps: usize) -> Self {
        let now = FlexibleTimestamp::now();
        Self {
            id: None,
            agent_id: agent_id.to_string(),
            goal: goal.to_string(),
            phase: RuntimePhase::Running,
            current_step: 0,
            max_steps,
            last_action: None,
            last_observation: None,
            history_tail: Vec::new(),
            error: None,
            started_at: now.clone(),
            updated_at: now,
            finished_at: None,
        }
    }
}
