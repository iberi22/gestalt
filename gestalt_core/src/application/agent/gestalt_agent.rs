use super::tools::create_gestalt_tools;
use super::AgentMode;
use crate::ports::outbound::repo_manager::{RepoManager, VectorDb};
use std::sync::Arc;
use synapse_agentic::prelude::*;
use synapse_agentic::framework::workflow::*;
use serde_json::json;

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
    pub async fn new(
        vector_db: Arc<dyn VectorDb>,
        repo_manager: Arc<dyn RepoManager>,
        llm_provider: Arc<dyn LLMProvider>,
    ) -> Self {
        let registry =
            create_gestalt_tools(repo_manager.clone(), vector_db.clone(), llm_provider).await;
        Self {
            _vector_db: vector_db,
            _repo_manager: repo_manager,
            _mode: AgentMode::Build,
            registry,
        }
    }
}

/// Adapter node that executes the Decision Engine for Gestalt.
#[derive(Debug)]
struct GestaltNode {
    id: String,
    engine: DecisionEngine,
    registry: ToolRegistry,
    repo_url: String,
    question: String,
}

#[async_trait]
impl GraphNode for GestaltNode {
    /// Returns the unique ID of the reasoner node.
    fn id(&self) -> &str {
        &self.id
    }

    /// Primary execution loop for the Gestalt reasoning process.
    ///
    /// Reads previous reasoning history and critic feedback from the state,
    /// then uses the `DecisionEngine` to decide on the next action (tool call or final answer).
    async fn execute(&mut self, state: &mut ContextState) -> anyhow::Result<NodeResult> {
        let history = state.get_string("reasoning_history").unwrap_or_default();
        let critic_feedback = state.get_string("critic_feedback").unwrap_or_default();

        // Build context with history and feedback
        let mut context = DecisionContext::new(&self.question)
            .with_metadata("repo_url", self.repo_url.clone())
            .with_metadata("history", history.clone());

        if !critic_feedback.is_empty() {
             context = context.with_metadata("critic_feedback", critic_feedback);
        }

        let decision = self.engine.decide(&context).await?;

        if let Some(tool_name) = decision.action.strip_prefix("call:") {
            tracing::info!("Agent decided to call tool: {}", tool_name);
            let args = decision.parameters.unwrap_or(json!({}));

            match self.registry.call(tool_name, &EmptyContext, args).await {
                Ok(result) => {
                    let mut updated_history = history;
                    updated_history.push_str(&format!("\nTool {}: {}\n", tool_name, result));
                    state.set_value("reasoning_history", json!(updated_history));
                    Ok(NodeResult::Continue(None))
                }
                Err(e) => {
                    // Capture error for the ReflectionNode to handle
                    Ok(NodeResult::Error(format!("Tool {} failed: {}", tool_name, e)))
                }
            }
        } else {
            tracing::info!("Agent provided final answer.");
            println!("🤖 {}", decision.action);
            state.set_value("final_answer", json!(decision.action));
            Ok(NodeResult::Halt)
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
                tracing::info!("Agent starting StateGraph flow for: {}", question);

                // 1. Setup Decision Engine
                let engine = DecisionEngine::builder()
                    .with_provider(GeminiProvider::new(
                        std::env::var("GEMINI_API_KEY").unwrap_or_default(),
                        "gemini-1.5-pro".to_string(),
                    ))
                    .build();

                // 2. Create the Node and Graph
                let gestalt_node = GestaltNode {
                    id: "gestalt_reasoner".to_string(),
                    engine,
                    registry: self.registry.clone(),
                    repo_url,
                    question,
                };

                // Critic will catch errors and route back to reasoner up to 10 times
                let critic = ReflectionNode::new("critic", "gestalt_reasoner", 10);

                let mut graph = StateGraph::new();
                graph.add_node(Box::new(gestalt_node));
                graph.add_node(Box::new(critic));

                graph.set_entry_point("gestalt_reasoner");
                graph.set_error_handler("critic");

                // 3. Execute
                let initial_state = ContextState::new(json!({}));
                let _final_state = graph.execute(initial_state).await?;

                Ok(())
            }
            _ => Ok(()),
        }
    }
}
