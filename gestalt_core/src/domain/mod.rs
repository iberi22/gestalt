use serde::{Deserialize, Serialize};
pub mod genui;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub model_name: String,
    pub content: String,
}

// Result of a query to multiple agents + synthesis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub individual_responses: Vec<AgentResponse>,
    pub synthesized_answer: String,
}
