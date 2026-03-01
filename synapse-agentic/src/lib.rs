pub mod prelude {
    pub use async_trait::async_trait;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::Value;

    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DecisionContext;
    impl DecisionContext {
        pub fn new(_q: &str) -> Self {
            Self
        }
        pub fn with_metadata(self, _k: &str, _v: String) -> Self {
            self
        }
        pub fn with_summary(self, _s: impl Into<String>) -> Self {
            self
        }
        pub fn with_data(self, _d: Value) -> Self {
            self
        }
    }

    #[async_trait]
    pub trait LLMProvider: Send + Sync + std::fmt::Debug {
        fn name(&self) -> &str;
        fn cost_per_1k_tokens(&self) -> f64;
        async fn generate(&self, prompt: &str) -> anyhow::Result<String>;
    }

    pub trait Provider: LLMProvider {}
    impl<T: LLMProvider> Provider for T {}

    #[derive(Debug, Clone)]
    pub struct GeminiProvider {
        model: String,
    }
    impl GeminiProvider {
        pub fn new(_key: String, model: String) -> Self {
            Self { model }
        }
    }
    #[async_trait]
    impl LLMProvider for GeminiProvider {
        fn name(&self) -> &str {
            &self.model
        }
        fn cost_per_1k_tokens(&self) -> f64 {
            0.0
        }
        async fn generate(&self, _prompt: &str) -> anyhow::Result<String> {
            Ok("mock".to_string())
        }
    }

    #[derive(Debug, Clone)]
    pub struct MinimaxProvider {
        model: String,
    }
    impl MinimaxProvider {
        pub fn new(_key: String, _group: String, model: String) -> Self {
            Self { model }
        }
    }
    #[async_trait]
    impl LLMProvider for MinimaxProvider {
        fn name(&self) -> &str {
            &self.model
        }
        fn cost_per_1k_tokens(&self) -> f64 {
            0.0
        }
        async fn generate(&self, _prompt: &str) -> anyhow::Result<String> {
            Ok("mock".to_string())
        }
    }

    #[derive(Debug, Clone)]
    pub struct InMemoryCooldownStore;
    impl Default for InMemoryCooldownStore {
        fn default() -> Self {
            Self::new()
        }
    }
    impl InMemoryCooldownStore {
        pub fn new() -> Self {
            Self
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProviderId {
        pub provider: String,
        pub model: String,
    }
    impl ProviderId {
        pub fn new(provider: &str, model: &str) -> Self {
            Self {
                provider: provider.to_string(),
                model: model.to_string(),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct StochasticRotator {
        providers: Vec<Arc<dyn LLMProvider>>,
    }
    impl StochasticRotator {
        pub fn new(_store: Arc<InMemoryCooldownStore>) -> Self {
            Self {
                providers: Vec::new(),
            }
        }
        pub fn add_provider(&mut self, _id: ProviderId, provider: Arc<dyn LLMProvider>) {
            self.providers.push(provider);
        }
    }
    #[async_trait]
    impl LLMProvider for StochasticRotator {
        fn name(&self) -> &str {
            "stochastic-rotator"
        }
        fn cost_per_1k_tokens(&self) -> f64 {
            0.0
        }
        async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
            if let Some(p) = self.providers.first() {
                p.generate(prompt).await
            } else {
                Ok("mock".to_string())
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Decision {
        pub reasoning: String,
        pub action: String,
        pub parameters: Option<Value>,
        pub confidence: f32,
        pub providers_used: Vec<String>,
    }

    #[derive(Debug, Clone)]
    pub struct DecisionEngine {
        providers: Vec<Arc<dyn LLMProvider>>,
    }
    impl Default for DecisionEngine {
        fn default() -> Self {
            Self::new()
        }
    }
    impl DecisionEngine {
        pub fn new() -> Self {
            Self {
                providers: Vec::new(),
            }
        }
        pub fn builder() -> DecisionEngineBuilder {
            DecisionEngineBuilder {
                providers: Vec::new(),
            }
        }
        pub fn providers(&self) -> &[Arc<dyn LLMProvider>] {
            &self.providers
        }
        pub async fn decide(&self, _ctx: &DecisionContext) -> anyhow::Result<Decision> {
            Ok(Decision {
                reasoning: "mock".to_string(),
                action: "final answer".to_string(),
                parameters: None,
                confidence: 1.0,
                providers_used: self
                    .providers
                    .iter()
                    .map(|p| p.name().to_string())
                    .collect(),
            })
        }
    }

    #[derive(Debug, Clone)]
    pub struct DecisionEngineBuilder {
        providers: Vec<Arc<dyn LLMProvider>>,
    }
    impl DecisionEngineBuilder {
        pub fn with_provider<P: Provider + 'static>(mut self, p: P) -> Self {
            self.providers.push(Arc::new(p));
            self
        }
        pub fn build(self) -> DecisionEngine {
            DecisionEngine {
                providers: self.providers,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EmptyContext;
    pub trait ToolContext {}
    impl ToolContext for EmptyContext {}

    #[derive(Debug, Clone)]
    pub struct ToolRegistry;
    impl Default for ToolRegistry {
        fn default() -> Self {
            Self::new()
        }
    }
    impl ToolRegistry {
        pub fn new() -> Self {
            Self
        }
        pub async fn register_tool<T: Tool + 'static>(&self, _tool: T) {}
        pub async fn call(
            &self,
            _name: &str,
            _ctx: &EmptyContext,
            _args: Value,
        ) -> anyhow::Result<Value> {
            Ok(Value::Null)
        }
    }

    #[async_trait]
    pub trait Tool: Send + Sync {
        fn name(&self) -> &str;
        fn description(&self) -> &str;
        fn parameters(&self) -> Value;
        async fn call(&self, ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value>;
    }

    #[async_trait]
    pub trait Agent: Send + Sync + 'static {
        type Input: Send + 'static;
        fn name(&self) -> &str;
        async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()>;
    }

    #[derive(Clone)]
    pub struct AgentHandle<T: Send + 'static> {
        tx: mpsc::Sender<T>,
    }
    impl<T: Send + 'static> AgentHandle<T> {
        pub async fn send(&self, msg: T) -> Result<(), mpsc::error::SendError<T>> {
            self.tx.send(msg).await
        }
    }

    #[derive(Default)]
    pub struct Hive;
    impl Hive {
        pub fn new() -> Self {
            Self
        }
        pub fn spawn<A>(&mut self, mut agent: A) -> AgentHandle<A::Input>
        where
            A: Agent,
        {
            let (tx, mut rx) = mpsc::channel::<A::Input>(64);
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    let _ = agent.handle(msg).await;
                }
            });
            AgentHandle { tx }
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
        async fn plan(
            &self,
            goal: &str,
            context: &DecisionContext,
        ) -> anyhow::Result<Vec<PlannedTask>>;
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
            let prompt = chunk
                .messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let summary = self.provider.generate(&prompt).await?;
            Ok(Message::new(MessageRole::Assistant, summary))
        }
    }
}

pub mod framework {
    pub mod workflow {
        use async_trait::async_trait;
        use serde_json::Value;
        use std::collections::HashMap;

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

            pub fn set_value(&mut self, key: &str, value: Value) {
                self.data.insert(key.to_string(), value);
            }
        }

        #[derive(Debug, Clone)]
        pub enum NodeResult {
            Continue(Option<String>),
            Error(String),
            Halt,
        }

        #[async_trait]
        pub trait GraphNode: Send + Sync {
            fn id(&self) -> &str;
            async fn execute(&mut self, state: &mut ContextState) -> anyhow::Result<NodeResult>;
        }

        pub struct ReflectionNode {
            id: String,
            route_to: String,
            retries: usize,
            current: usize,
        }

        impl ReflectionNode {
            pub fn new(id: &str, route_to: &str, retries: usize) -> Self {
                Self {
                    id: id.to_string(),
                    route_to: route_to.to_string(),
                    retries,
                    current: 0,
                }
            }
        }

        #[async_trait]
        impl GraphNode for ReflectionNode {
            fn id(&self) -> &str {
                &self.id
            }

            async fn execute(&mut self, _state: &mut ContextState) -> anyhow::Result<NodeResult> {
                if self.current < self.retries {
                    self.current += 1;
                    Ok(NodeResult::Continue(Some(self.route_to.clone())))
                } else {
                    Ok(NodeResult::Halt)
                }
            }
        }

        pub struct StateGraph {
            nodes: HashMap<String, Box<dyn GraphNode>>,
            entry: Option<String>,
            error_handler: Option<String>,
        }

        impl Default for StateGraph {
            fn default() -> Self {
                Self::new()
            }
        }

        impl StateGraph {
            pub fn new() -> Self {
                Self {
                    nodes: HashMap::new(),
                    entry: None,
                    error_handler: None,
                }
            }

            pub fn add_node(&mut self, node: Box<dyn GraphNode>) {
                self.nodes.insert(node.id().to_string(), node);
            }

            pub fn set_entry_point(&mut self, id: &str) {
                self.entry = Some(id.to_string());
            }

            pub fn set_error_handler(&mut self, id: &str) {
                self.error_handler = Some(id.to_string());
            }

            pub async fn execute(
                &mut self,
                mut state: ContextState,
            ) -> anyhow::Result<ContextState> {
                let mut current = self
                    .entry
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("entry point not configured"))?;

                loop {
                    let node = self
                        .nodes
                        .get_mut(&current)
                        .ok_or_else(|| anyhow::anyhow!("node '{}' not found", current))?;

                    match node.execute(&mut state).await? {
                        NodeResult::Halt => break,
                        NodeResult::Continue(Some(next)) => current = next,
                        NodeResult::Continue(None) => {}
                        NodeResult::Error(err) => {
                            state.set_value("error", Value::String(err));
                            if let Some(handler) = self.error_handler.clone() {
                                current = handler;
                            } else {
                                break;
                            }
                        }
                    }
                }

                Ok(state)
            }
        }
    }
}
