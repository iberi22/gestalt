use synapse_agentic::prelude::{
    CompactionConfig, ContextOverflowRisk, Message, MessageRole, SessionContext,
    SimpleTokenEstimator, TokenCounter, LLMSummarizer, LLMProvider,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CompactionOutcome {
    pub compacted: bool,
    pub tokens_before: u32,
    pub tokens_after: u32,
}

#[derive(Debug, Clone)]
pub struct ContextCompactor {
    estimator: SimpleTokenEstimator,
    config: CompactionConfig,
    summarizer: Arc<LLMSummarizer>,
}

impl ContextCompactor {
    pub fn new(provider: Arc<dyn LLMProvider>, model: &str) -> Self {
        Self {
            estimator: SimpleTokenEstimator::new(model),
            config: CompactionConfig::small_context(),
            summarizer: Arc::new(LLMSummarizer::for_technical(provider)),
        }
    }

    pub async fn compact(&self, history: &mut Vec<String>) -> CompactionOutcome {
        let mut session = SessionContext::new(self.config.clone());

        for item in history.iter() {
            let mut message = Message::new(MessageRole::Assistant, item.clone());
            if let Ok(tokens) = self.estimator.count_message(&message) {
                message.token_count = Some(tokens);
            }
            session.add_message(message);
        }

        let tokens_before = session.total_tokens();
        if !matches!(
            session.overflow_risk(),
            ContextOverflowRisk::Warning | ContextOverflowRisk::Critical
        ) {
            return CompactionOutcome {
                compacted: false,
                tokens_before,
                tokens_after: tokens_before,
            };
        }

        let compactable_messages = session.compactable_messages();
        if compactable_messages.is_empty() {
            return CompactionOutcome {
                compacted: false,
                tokens_before,
                tokens_after: tokens_before,
            };
        }

        let compactable_len = compactable_messages.len();
        let chunk = synapse_agentic::prelude::MessageChunk::new(
            compactable_messages.to_vec(),
            0,
        );

        match self.summarizer.summarize(&chunk).await {
            Ok(summary_msg) => {
                let keep_recent = session.recent_messages().len();
                let mut next = Vec::with_capacity(keep_recent + 1);
                next.push(summary_msg.content);
                next.extend_from_slice(&history[compactable_len..]);
                *history = next;

                let tokens_after = history
                    .iter()
                    .filter_map(|line| self.estimator.count_tokens(line).ok())
                    .sum::<u32>();

                CompactionOutcome {
                    compacted: true,
                    tokens_before,
                    tokens_after,
                }
            }
            Err(e) => {
                tracing::error!("Context compaction failed: {}", e);
                CompactionOutcome {
                    compacted: false,
                    tokens_before,
                    tokens_after: tokens_before,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[derive(Debug)]
    struct MockProvider;

    #[async_trait]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn cost_per_1k_tokens(&self) -> f64 {
            0.0
        }
        async fn generate(&self, _prompt: &str) -> anyhow::Result<String> {
            Ok("Summary from mock provider".to_string())
        }
    }

    #[tokio::test]
    async fn compacts_when_history_is_large() {
        let provider = Arc::new(MockProvider);
        let compactor = ContextCompactor::new(provider, "gpt-4o");
        let mut history = Vec::new();

        for i in 0..80 {
            history.push(format!(
                "Action {} Observation with verbose payload {}",
                i,
                "x".repeat(120)
            ));
        }

        let outcome = compactor.compact(&mut history).await;
        assert!(outcome.compacted);
        assert_eq!(history.first().unwrap(), "Summary from mock provider");
    }
}
