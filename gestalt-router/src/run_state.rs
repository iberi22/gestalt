use crate::run::RunSpec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub use crate::run::AgentStatus as AgentState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: Uuid,
    pub spec: RunSpec,
    pub agent_states: HashMap<String, AgentState>,
}
