use crate::providers::LLMProvider;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    pub query: String,
    pub summary: Option<String>,
    pub metadata: HashMap<String, String>,
    pub data: Option<Value>,
}

impl DecisionContext {
    pub fn new(q: &str) -> Self {
        Self {
            query: q.to_string(),
            summary: None,
            metadata: HashMap::new(),
            data: None,
        }
    }
    pub fn with_metadata(mut self, k: &str, v: String) -> Self {
        self.metadata.insert(k.to_string(), v);
        self
    }
    pub fn with_summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }
    pub fn with_data(mut self, d: Value) -> Self {
        self.data = Some(d);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ContextState {
    data: HashMap<String, Value>,
}

impl ContextState {
    pub fn new(initial: Value) -> Self {
        let mut data = HashMap::new();
        if let Value::Object(map) = initial {
            for (k, v) in map {
                data.insert(k, v);
            }
        }
        Self { data }
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.data
            .get(key)
            .and_then(|v| v.as_str().map(ToOwned::to_owned))
    }

    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn set_value(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub token_count: Option<u32>,
}

impl Message {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            role,
            content,
            token_count: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageChunk {
    pub messages: Vec<Message>,
    pub start_index: usize,
}

impl MessageChunk {
    pub fn new(messages: Vec<Message>, start_index: usize) -> Self {
        Self {
            messages,
            start_index,
        }
    }
}

pub trait TokenCounter: Send + Sync {
    fn count_tokens(&self, text: &str) -> anyhow::Result<u32>;
    fn count_message(&self, message: &Message) -> anyhow::Result<u32> {
        self.count_tokens(&message.content)
    }
}

#[derive(Debug, Clone)]
pub struct SimpleTokenEstimator;

impl SimpleTokenEstimator {
    pub fn new(_model: &str) -> Self {
        Self
    }
}

impl TokenCounter for SimpleTokenEstimator {
    fn count_tokens(&self, text: &str) -> anyhow::Result<u32> {
        Ok(text.split_whitespace().count() as u32)
    }
}

#[derive(Debug, Clone)]
pub struct CompactionConfig {
    pub warning_tokens: u32,
    pub critical_tokens: u32,
    pub keep_recent: usize,
}

impl CompactionConfig {
    pub fn small_context() -> Self {
        Self {
            warning_tokens: 1500,
            critical_tokens: 2500,
            keep_recent: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextOverflowRisk {
    Low,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    pub id: String,
    pub description: String,
    pub estimated_tool: Option<String>,
    pub status: TaskStatus,
}

#[async_trait]
pub trait ExplicitPlanner: Send + Sync {
    async fn plan(&self, goal: &str, context: &DecisionContext)
        -> anyhow::Result<Vec<PlannedTask>>;
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    cfg: CompactionConfig,
    messages: Vec<Message>,
}

impl SessionContext {
    pub fn new(cfg: CompactionConfig) -> Self {
        Self {
            cfg,
            messages: Vec::new(),
        }
    }
    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }
    pub fn total_tokens(&self) -> u32 {
        self.messages
            .iter()
            .map(|m| m.token_count.unwrap_or(0))
            .sum()
    }
    pub fn overflow_risk(&self) -> ContextOverflowRisk {
        let total = self.total_tokens();
        if total >= self.cfg.critical_tokens {
            ContextOverflowRisk::Critical
        } else if total >= self.cfg.warning_tokens {
            ContextOverflowRisk::Warning
        } else {
            ContextOverflowRisk::Low
        }
    }
    pub fn compactable_messages(&self) -> &[Message] {
        if self.messages.len() > self.cfg.keep_recent {
            &self.messages[..self.messages.len() - self.cfg.keep_recent]
        } else {
            &[]
        }
    }
    pub fn recent_messages(&self) -> &[Message] {
        let keep = self.cfg.keep_recent.min(self.messages.len());
        &self.messages[self.messages.len().saturating_sub(keep)..]
    }
    pub fn recent_messages_mut(&mut self) -> &mut [Message] {
        let keep = self.cfg.keep_recent.min(self.messages.len());
        let start = self.messages.len().saturating_sub(keep);
        &mut self.messages[start..]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SummarizationStrategy {
    Technical,
}

#[derive(Debug, Clone)]
pub struct LLMSummarizer {
    provider: Arc<dyn LLMProvider>,
    _strategy: SummarizationStrategy,
}

impl LLMSummarizer {
    pub fn for_technical(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            _strategy: SummarizationStrategy::Technical,
        }
    }
    pub async fn summarize(&self, chunk: &MessageChunk) -> anyhow::Result<Message> {
        let history = chunk
            .messages
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "Summarize the following technical conversation history concisely, focusing on actions taken and their outcomes:\n\n{}",
            history
        );
        let summary = self.provider.generate(&prompt).await?;
        Ok(Message::new(MessageRole::Assistant, summary))
    }
}
