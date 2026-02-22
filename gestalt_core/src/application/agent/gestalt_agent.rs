use super::tools::create_gestalt_tools;
use super::AgentMode;
use crate::ports::outbound::repo_manager::{RepoManager, VectorDb};
use std::sync::Arc;
use synapse_agentic::prelude::*;

#[derive(Debug)]
pub enum GestaltInput {
    Ask { repo_url: String, question: String },
    Index { url: String },
    Execute { command: String },
    SetMode(AgentMode),
}

pub struct GestaltAgent {
    _vector_db: Arc<dyn VectorDb>,
    _repo_manager: Arc<dyn RepoManager>,
    _mode: AgentMode,
    registry: ToolRegistry,
}

impl GestaltAgent {
    pub async fn new(vector_db: Arc<dyn VectorDb>, repo_manager: Arc<dyn RepoManager>) -> Self {
        let registry = create_gestalt_tools(repo_manager.clone(), vector_db.clone()).await;
        Self {
            _vector_db: vector_db,
            _repo_manager: repo_manager,
            _mode: AgentMode::Build,
            registry,
        }
    }
}

#[async_trait]
impl Agent for GestaltAgent {
    type Input = GestaltInput;

    fn name(&self) -> &str {
        "gestalt-agent"
    }

    async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()> {
        match msg {
            GestaltInput::Ask { repo_url, question } => {
                tracing::info!("Agent searching for answer: {}", question);

                // 1. Initialize Decision Engine using Synapse framework
                let engine = DecisionEngine::builder()
                    .with_provider(GeminiProvider::new(
                        std::env::var("GEMINI_API_KEY").unwrap_or_default(),
                        "gemini-1.5-pro".to_string(),
                    ))
                    .build();

                // 2. Main reasoning loop
                let mut history = String::new();
                for _ in 0..5 {
                    // Limit to 5 iterations
                    let context = DecisionContext::new(&question)
                        .with_metadata("repo_url", repo_url.clone())
                        .with_metadata("history", history.clone());

                    let decision = engine.decide(&context).await?;

                    if let Some(tool_name) = decision.action.strip_prefix("call:") {
                        tracing::info!("Agent decided to call tool: {}", tool_name);
                        let args = decision.parameters.unwrap_or(serde_json::json!({}));
                        let result = self.registry.call(tool_name, &EmptyContext, args).await?;
                        history.push_str(&format!("\nTool {}: {}\n", tool_name, result));
                    } else {
                        tracing::info!("Agent provided final answer.");
                        println!("🤖 {}", decision.action);
                        break;
                    }
                }
                Ok(())
            }
            _ => Ok(()), // Other cases handled by basic logic or state machine
        }
    }
}
