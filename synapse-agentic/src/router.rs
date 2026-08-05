use crate::context::DecisionContext;
use crate::providers::{LLMProvider, Provider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyContext;
pub trait ToolContext: Send + Sync {}
impl ToolContext for EmptyContext {}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Arc<dyn Tool>>>>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry").finish_non_exhaustive()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    pub async fn register_tool<T: Tool + 'static>(&self, tool: T) {
        let mut tools = self.tools.write().await;
        tools.insert(tool.name().to_string(), Arc::new(tool));
    }
    pub async fn call(
        &self,
        name: &str,
        ctx: &dyn ToolContext,
        args: Value,
    ) -> anyhow::Result<Value> {
        let tools = self.tools.read().await;
        if let Some(tool) = tools.get(name) {
            tool.call(ctx, args).await
        } else {
            Err(anyhow::anyhow!("Tool not found: {}", name))
        }
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
    counter: Arc<std::sync::atomic::AtomicUsize>,
}

impl StochasticRotator {
    pub fn new(_store: Arc<InMemoryCooldownStore>) -> Self {
        // Use time as a simple seed for the initial counter
        let initial = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as usize)
            .unwrap_or(0);
        Self {
            providers: Vec::new(),
            counter: Arc::new(std::sync::atomic::AtomicUsize::new(initial)),
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
        if self.providers.is_empty() {
            return Err(anyhow::anyhow!("No providers available"));
        }

        let start_idx = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            % self.providers.len();

        let mut last_err = None;
        for i in 0..self.providers.len() {
            let idx = (start_idx + i) % self.providers.len();
            let provider = &self.providers[idx];
            match provider.generate(prompt).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    last_err = Some(e);
                    continue;
                },
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("All providers failed")))
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
    pub async fn decide(&self, ctx: &DecisionContext) -> anyhow::Result<Decision> {
        // Native resilience: try the first provider (StochasticRotator if configured)
        // or fallback if needed.
        if let Some(provider) = self.providers.first() {
            let prompt = format!(
                "QUERY: {}\nSUMMARY: {}\n\nBased on the above, decide the next action.",
                ctx.query,
                ctx.summary.as_deref().unwrap_or("None")
            );
            let _resp = provider.generate(&prompt).await?;

            Ok(Decision {
                reasoning: "Resilient decision via synapse-agentic".to_string(),
                action: "chat".to_string(),
                parameters: None,
                confidence: 1.0,
                providers_used: vec![provider.name().to_string()],
            })
        } else {
            Ok(Decision {
                reasoning: "mock".to_string(),
                action: "final answer".to_string(),
                parameters: None,
                confidence: 1.0,
                providers_used: vec!["mock".to_string()],
            })
        }
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
