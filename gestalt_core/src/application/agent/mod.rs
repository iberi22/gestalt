pub mod gestalt_agent;
pub mod registry;
pub mod tools;
pub mod xavier;

pub use gestalt_agent::{GestaltAgent, GestaltInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentMode {
    Build,
    Research,
    Consensus,
}
