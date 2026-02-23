use synapse_agentic::prelude::{
    CompactionConfig, ContextOverflowRisk, Message, MessageRole, SessionContext,
    SimpleTokenEstimator, TokenCounter,
};

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
}

impl ContextCompactor {
    pub fn new(model: &str) -> Self {
        Self {
            estimator: SimpleTokenEstimator::new(model),
            config: CompactionConfig::small_context(),
        }
    }

    pub fn compact(&self, history: &mut Vec<String>) -> CompactionOutcome {
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

        let compactable_len = session.compactable_messages().len();
        if compactable_len == 0 {
            return CompactionOutcome {
                compacted: false,
                tokens_before,
                tokens_after: tokens_before,
            };
        }

        let keep_recent = session.recent_messages().len();
        let summary_input = history[..compactable_len].join("\n");
        let summary = self.build_summary(&summary_input, compactable_len);

        let mut next = Vec::with_capacity(keep_recent + 1);
        next.push(summary);
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

    fn build_summary(&self, text: &str, lines: usize) -> String {
        let max_chars = 1200usize;
        let mut compacted = text.to_string();
        if compacted.len() > max_chars {
            compacted.truncate(max_chars);
            compacted.push_str("...");
        }
        format!("[ContextSummary lines={}]\n{}", lines, compacted)
    }
}

#[cfg(test)]
mod tests {
    use super::ContextCompactor;

    #[test]
    fn compacts_when_history_is_large() {
        let compactor = ContextCompactor::new("gpt-4o");
        let mut history = Vec::new();

        for i in 0..80 {
            history.push(format!(
                "Action {} Observation with verbose payload {}",
                i,
                "x".repeat(120)
            ));
        }

        let outcome = compactor.compact(&mut history);
        assert!(outcome.compacted);
        assert!(history.first().unwrap().starts_with("[ContextSummary"));
    }
}
