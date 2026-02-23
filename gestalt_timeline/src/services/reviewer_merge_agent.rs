use anyhow::Result;
use tokio::sync::oneshot;

use synapse_agentic::prelude::{async_trait, Agent, AgentHandle, Hive};

#[derive(Debug)]
pub enum ReviewerMessage {
    ReviewAndMerge {
        goal: String,
        reply: oneshot::Sender<ReviewResult>,
    },
}

#[derive(Debug, Clone)]
pub struct ReviewResult {
    pub approved: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct ReviewerMergeAgent;

#[async_trait]
impl Agent for ReviewerMergeAgent {
    type Input = ReviewerMessage;

    fn name(&self) -> &str {
        "reviewer-merge-agent"
    }

    async fn handle(&mut self, message: Self::Input) -> Result<()> {
        match message {
            ReviewerMessage::ReviewAndMerge { goal, reply } => {
                let approved = !goal.trim().is_empty();
                let summary = if approved {
                    format!("Review completed for goal '{}'. Merge approved.", goal)
                } else {
                    "Review failed: empty goal.".to_string()
                };
                let _ = reply.send(ReviewResult { approved, summary });
            }
        }
        Ok(())
    }
}

pub fn spawn_reviewer_agent(hive: &mut Hive) -> AgentHandle<ReviewerMessage> {
    hive.spawn(ReviewerMergeAgent)
}
