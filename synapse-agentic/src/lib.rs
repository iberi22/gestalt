pub mod prelude {
    pub use serde::{Serialize, Deserialize};
    pub use async_trait::async_trait;
    pub use serde_json::Value;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DecisionContext;
    impl DecisionContext {
        pub fn new(_q: &str) -> Self { Self }
        pub fn with_metadata(self, _k: &str, _v: String) -> Self { self }
        pub fn with_summary(self, _s: impl Into<String>) -> Self { self }
        pub fn with_data(self, _d: Value) -> Self { self }
    }

    pub struct DecisionEngine;
    impl DecisionEngine {
        pub fn builder() -> DecisionEngineBuilder { DecisionEngineBuilder }
        pub async fn decide(&self, _ctx: &DecisionContext) -> anyhow::Result<Decision> {
             Ok(Decision { reasoning: "mock".to_string(), action: "final answer".to_string(), parameters: None, confidence: 1.0, providers_used: vec![] })
        }
    }

    pub struct DecisionEngineBuilder;
    impl DecisionEngineBuilder {
        pub fn with_provider(self, _p: impl Provider) -> Self { self }
        pub fn build(self) -> DecisionEngine { DecisionEngine }
    }

    pub trait Provider {}

    pub struct GeminiProvider;
    impl GeminiProvider {
        pub fn new(_key: String, _model: String) -> Self { Self }
    }
    impl Provider for GeminiProvider {}

    pub struct MinimaxProvider;
    impl MinimaxProvider {
        pub fn new(_key: String, _group: String, _model: String) -> Self { Self }
    }
    impl Provider for MinimaxProvider {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Decision {
        pub reasoning: String,
        pub action: String,
        pub parameters: Option<Value>,
        pub confidence: f32,
        pub providers_used: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EmptyContext;
    pub struct ToolRegistry;
    impl ToolRegistry {
        pub fn new() -> Self { Self }
        pub async fn register_tool<T: Tool + 'static>(&self, _tool: T) {}
        pub async fn call(&self, _name: &str, _ctx: &EmptyContext, _args: Value) -> anyhow::Result<Value> {
            Ok(Value::Null)
        }
    }

    #[async_trait]
    pub trait Agent {
        type Input;
        fn name(&self) -> &str;
        async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()>;
    }

    #[async_trait]
    pub trait Tool: Send + Sync {
        fn name(&self) -> &str;
        fn description(&self) -> &str;
        fn parameters(&self) -> Value;
        async fn call(&self, ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value>;
    }

    pub trait ToolContext {}
    impl ToolContext for EmptyContext {}

    pub trait Agentic {
        // Mock traits as needed
    }
}
