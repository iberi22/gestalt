pub mod prelude {
    pub use async_trait::async_trait;
    pub use serde_json::Value;
    pub use anyhow::Result;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct DecisionEngine;
    impl DecisionEngine {
        pub fn new() -> Self { Self }
        pub fn builder() -> DecisionEngineBuilder { DecisionEngineBuilder }
        pub async fn decide(&self, _ctx: &DecisionContext) -> Result<Decision> { Ok(Decision::default()) }
    }
    pub struct DecisionEngineBuilder;
    impl DecisionEngineBuilder {
        pub fn with_provider<P>(self, _p: P) -> Self { self }
        pub fn build(self) -> DecisionEngine { DecisionEngine }
    }
    pub struct Decision {
        pub action: String,
        pub reasoning: String,
        pub confidence: f32,
        pub providers_used: Vec<String>,
        pub parameters: Option<Value>,
    }
    impl Default for Decision {
        fn default() -> Self {
            Self {
                action: "stop".to_string(),
                reasoning: "default".to_string(),
                confidence: 1.0,
                providers_used: vec![],
                parameters: None,
            }
        }
    }

    pub struct ToolRegistry {
        tools: Arc<RwLock<HashMap<String, Box<dyn Tool>>>>,
    }
    impl Default for ToolRegistry {
        fn default() -> Self {
            Self {
                tools: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }
    impl ToolRegistry {
        pub fn new() -> Self { Self::default() }
        pub async fn register_tool<T: Tool + 'static>(&self, tool: T) {
            self.tools.write().await.insert(tool.name().to_string(), Box::new(tool));
        }
        pub async fn call(&self, name: &str, ctx: &dyn ToolContext, args: Value) -> Result<Value> {
            let tools = self.tools.read().await;
            if let Some(tool) = tools.get(name) {
                tool.call(ctx, args).await
            } else {
                Err(anyhow::anyhow!("Tool not found: {}", name))
            }
        }
    }

    pub struct DecisionContext {
        pub summary: String,
    }
    impl DecisionContext {
        pub fn new(summary: &str) -> Self { Self { summary: summary.to_string() } }
        pub fn with_metadata(self, _k: &str, _v: String) -> Self { self }
        pub fn with_summary(mut self, _s: impl Into<String>) -> Self {
            self.summary = _s.into();
            self
        }
        pub fn with_data(self, _data: Value) -> Self { self }
    }

    pub struct EmptyContext;
    pub trait ToolContext: Send + Sync {}
    impl ToolContext for EmptyContext {}

    #[async_trait]
    pub trait Tool: Send + Sync {
        fn name(&self) -> &str;
        fn description(&self) -> &str;
        fn parameters(&self) -> Value;
        async fn call(&self, ctx: &dyn ToolContext, args: Value) -> Result<Value>;
    }

    #[async_trait]
    pub trait Agent: Send + Sync {
        type Input;
        fn name(&self) -> &str;
        async fn handle(&mut self, msg: Self::Input) -> Result<()>;
    }

    pub struct MinimaxProvider;
    impl MinimaxProvider {
        pub fn new(_key: String, _group: String, _model: String) -> Self { Self }
    }
    pub struct GeminiProvider;
    impl GeminiProvider {
        pub fn new(_key: String, _model: String) -> Self { Self }
    }
}
