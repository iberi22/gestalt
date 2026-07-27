use serde::{Deserialize, Serialize};
pub mod error;
pub mod genui;
pub mod rag;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentResponse {
    pub model_name: String,
    pub content: String,
}

// Result of a query to multiple agents + synthesis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsensusResult {
    pub individual_responses: Vec<AgentResponse>,
    pub synthesized_answer: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        let roles = vec![Role::System, Role::User, Role::Assistant];
        for role in roles {
            let serialized = serde_json::to_string(&role).unwrap();
            let deserialized: Role = serde_json::from_str(&serialized).unwrap();
            assert_eq!(role, deserialized);
        }
    }

    #[test]
    fn test_message_struct() {
        let msg = Message {
            role: Role::User,
            content: "Generate a Rust trait".to_string(),
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&serialized).unwrap();
        assert_eq!(msg.role, deserialized.role);
        assert_eq!(msg.content, deserialized.content);
    }

    #[test]
    fn test_consensus_result_roundtrip() {
        let consensus = ConsensusResult {
            individual_responses: vec![
                AgentResponse {
                    model_name: "gpt-4o".to_string(),
                    content: "Option A is optimal".to_string(),
                },
                AgentResponse {
                    model_name: "claude-3-5-sonnet".to_string(),
                    content: "Option A works best".to_string(),
                },
            ],
            synthesized_answer: "The consensus is Option A.".to_string(),
        };

        let serialized = serde_json::to_string(&consensus).unwrap();
        let deserialized: ConsensusResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(consensus, deserialized);
    }
}
