pub mod prelude {
    pub use async_trait::async_trait;
    pub use crate::{Agent, DecisionEngine, GeminiProvider, MinimaxProvider, DecisionContext, ToolRegistry, Tool, ToolContext, EmptyContext};
}

use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Agent {
    type Input;
    fn name(&self) -> &str;
    async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()>;
}

pub struct DecisionEngine;
impl DecisionEngine {
    pub fn new() -> Self {
        DecisionEngine
    }
    pub fn builder() -> DecisionEngineBuilder {
        DecisionEngineBuilder
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DecisionEngineBuilder;
impl DecisionEngineBuilder {
    pub fn with_provider<T>(self, _provider: T) -> Self {
        self
    }
    pub fn build(self) -> DecisionEngine {
        DecisionEngine
    }
}

impl DecisionEngine {
    pub async fn decide(&self, _ctx: &DecisionContext) -> anyhow::Result<Decision> {
        Ok(Decision {
            action: "I am a mock decision".to_string(),
            reasoning: "I am reasoning".to_string(),
            parameters: None,
            confidence: 1.0,
            providers_used: vec![],
        })
    }
}

pub struct GeminiProvider;
impl GeminiProvider {
    pub fn new(_key: String, _model: String) -> Self {
        GeminiProvider
    }
}

pub struct MinimaxProvider;
impl MinimaxProvider {
    pub fn new(_key: String, _group_id: String, _model: String) -> Self {
        MinimaxProvider
    }
}

pub struct DecisionContext;
impl DecisionContext {
    pub fn new(_question: &str) -> Self {
        DecisionContext
    }
    pub fn with_metadata(self, _key: &str, _value: String) -> Self {
        self
    }
    pub fn with_summary<T: AsRef<str>>(self, _summary: T) -> Self {
        self
    }
    pub fn with_data(self, _data: Value) -> Self {
        self
    }
}

pub struct Decision {
    pub action: String,
    pub reasoning: String,
    pub parameters: Option<Value>,
    pub confidence: f32,
    pub providers_used: Vec<String>,
}

pub struct ToolRegistry;
impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry
    }
    pub async fn register_tool<T: Tool + 'static>(&self, _tool: T) {}
    pub async fn call(&self, _name: &str, _ctx: &dyn ToolContext, _args: Value) -> anyhow::Result<Value> {
        Ok(serde_json::json!({}))
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn call(&self, ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value>;
}

pub trait ToolContext: Send + Sync {}
pub struct EmptyContext;
impl ToolContext for EmptyContext {}
