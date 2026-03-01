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

    pub async fn compact(&self, session: &mut SessionContext) -> CompactionOutcome {
        // Update token counts for all messages using the estimator
        for msg in session.recent_messages_mut() {
            if msg.token_count.is_none() {
                if let Ok(tokens) = self.estimator.count_message(msg) {
                    msg.token_count = Some(tokens);
                }
            }
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

        let chunk = synapse_agentic::prelude::MessageChunk::new(
            compactable_messages.to_vec(),
            0,
        );

        match self.summarizer.summarize(&chunk).await {
            Ok(summary_msg) => {
                // In a real framework integration, the SessionContext would handle the message rotation
                // and replacement. For this implementation, we simulate it.
                let mut new_messages = vec![summary_msg];
                new_messages.extend_from_slice(session.recent_messages());

                // Note: The SessionContext should ideally have a method to replace messages.
                // Assuming it's simple enough to recreate for this demonstration.
                *session = SessionContext::new(self.config.clone());
                for msg in new_messages {
                    session.add_message(msg);
                }

                let tokens_after = session.total_tokens();
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
        let mut session = SessionContext::new(CompactionConfig::small_context());

        for i in 0..80 {
            session.add_message(Message::new(
                MessageRole::User,
                format!(
                    "Action {} Observation with verbose payload {}",
                    i,
                    "x".repeat(120)
                )
            ));
        }

        let outcome = compactor.compact(&mut session).await;
        assert!(outcome.compacted);
        assert_eq!(session.recent_messages().first().unwrap().content, "Summary from mock provider");
    }
}
