use super::tools::create_gestalt_tools;
use super::AgentMode;
use crate::ports::outbound::repo_manager::{RepoManager, VectorDb};
use serde_json::json;
use std::sync::Arc;
use synapse_agentic::framework::workflow::*;
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

        // Extract current plan progress
        let plan_val = state.get_value("plan").unwrap_or(json!([]));
        let step_index = state
            .get_value("current_step_index")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let plan: Vec<PlannedTask> = serde_json::from_value(plan_val.clone())?;

        if step_index >= plan.len() {
            tracing::info!("All planned steps completed.");
            state.set_value("final_answer", json!("Goal achieved according to plan."));
            return Ok(NodeResult::Halt);
        }

        let current_task = &plan[step_index];
        tracing::info!(
            "Executing step {}/{}: {}",
            step_index + 1,
            plan.len(),
            current_task.description
        );

        // Build context with history, feedback, and CURRENT PLAN STEP
        let mut context = DecisionContext::new(&self.question)
            .with_metadata("repo_url", self.repo_url.clone())
            .with_metadata("history", history.clone())
            .with_metadata("current_step", current_task.description.clone())
            .with_metadata("total_steps", plan.len().to_string())
            .with_metadata("step_index", step_index.to_string());

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
                    updated_history.push_str(&format!(
                        "\nStep {}: {} -> Tool {}: {}\n",
                        step_index + 1,
                        current_task.description,
                        tool_name,
                        result
                    ));
                    state.set_value("reasoning_history", json!(updated_history));

                    // Move to next step in the plan
                    state.set_value("current_step_index", json!(step_index + 1));

                    Ok(NodeResult::Continue(None))
                }
                Err(e) => {
                    // Capture error for the ReflectionNode to handle
                    Ok(NodeResult::Error(format!(
                        "Step {} failed at tool {}: {}",
                        step_index + 1,
                        tool_name,
                        e
                    )))
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
                    engine: engine.clone(),
                    registry: self.registry.clone(),
                    repo_url: repo_url.clone(),
                    question: question.clone(),
                };

                // Critic will catch errors and route back to reasoner up to 10 times
                let critic = ReflectionNode::new("critic", "gestalt_reasoner", 10);

                let mut graph = StateGraph::new();
                graph.add_node(Box::new(gestalt_node));
                graph.add_node(Box::new(critic));

                graph.set_entry_point("gestalt_reasoner");
                graph.set_error_handler("critic");

                // 3. Planning phase (Explicit Planning)
                let context = DecisionContext::new(&question).with_metadata("repo_url", repo_url);
                let plan = self.plan(&question, &context).await?;
                tracing::info!("Generated plan with {} steps", plan.len());

                // 4. Execute
                let initial_state = ContextState::new(json!({
                    "plan": plan,
                    "current_step_index": 0
                }));
                let _final_state = graph.execute(initial_state).await?;

                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl ExplicitPlanner for GestaltAgent {
    async fn plan(
        &self,
        goal: &str,
        context: &DecisionContext,
    ) -> anyhow::Result<Vec<PlannedTask>> {
        // In a real implementation, we would use an LLM to generate the plan.
        // For now, we simulate the structured planning as per the architectural proposal.
        tracing::info!("Planning for goal: {}", goal);

        // This would call the DecisionEngine with a specific planning prompt
        // For this task, we provide a mock structured plan that reflects the "Plan-first" approach.
        let tasks = vec![
            PlannedTask {
                id: "1".to_string(),
                description: format!("Analyze requirements for: {}", goal),
                estimated_tool: Some("scan_workspace".to_string()),
                status: TaskStatus::Pending,
            },
            PlannedTask {
                id: "2".to_string(),
                description: "Execute necessary actions".to_string(),
                estimated_tool: None,
                status: TaskStatus::Pending,
            },
            PlannedTask {
                id: "3".to_string(),
                description: "Verify results".to_string(),
                estimated_tool: Some("execute_shell".to_string()),
                status: TaskStatus::Pending,
            },
        ];

        Ok(tasks)
    }
}
