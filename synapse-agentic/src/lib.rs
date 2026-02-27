pub mod prelude {
    pub use async_trait::async_trait;
    pub use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[async_trait]
    pub trait Agent {
        type Input;
        fn name(&self) -> &str;
        async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()>;
    }

    pub struct DecisionEngine;
    impl Default for DecisionEngine {
        fn default() -> Self {
            Self::new()
        }
    }
    impl DecisionEngine {
        pub fn new() -> Self { Self }
        pub fn builder() -> DecisionEngineBuilder { DecisionEngineBuilder }
        pub async fn decide(&self, _ctx: &DecisionContext) -> anyhow::Result<Decision> {
            Ok(Decision {
                action: "mock".to_string(),
                parameters: None,
                reasoning: "mock reasoning".to_string(),
                confidence: 1.0,
                providers_used: vec![],
            })
        }
    }

    pub struct DecisionEngineBuilder;
    impl DecisionEngineBuilder {
        pub fn with_provider<P>(self, _provider: P) -> Self { self }
        pub fn build(self) -> DecisionEngine { DecisionEngine }
    }

    pub struct DecisionContext {
        pub query: String,
    }
    impl DecisionContext {
        pub fn new(query: &str) -> Self { Self { query: query.to_string() } }
        pub fn with_metadata(self, _key: &str, _value: String) -> Self { self }
        pub fn with_summary(self, _summary: impl AsRef<str>) -> Self { self }
        pub fn with_data(self, _data: Value) -> Self { self }
    }

    pub struct Decision {
        pub action: String,
        pub parameters: Option<Value>,
        pub reasoning: String,
        pub confidence: f32,
        pub providers_used: Vec<String>,
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

    pub struct ToolRegistry {
        tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    }

    impl Default for ToolRegistry {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ToolRegistry {
        pub fn new() -> Self {
            Self {
                tools: RwLock::new(HashMap::new()),
            }
        }
        pub async fn register_tool<T: Tool + 'static>(&self, tool: T) {
            let mut tools = self.tools.write().await;
            tools.insert(tool.name().to_string(), Arc::new(tool));
        }
        pub async fn call(&self, name: &str, ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
            let tools = self.tools.read().await;
            let tool = tools.get(name).cloned();
            drop(tools);

            if let Some(tool) = tool {
                tool.call(ctx, args).await
            } else {
                Err(anyhow::anyhow!("Tool not found: {}", name))
            }
        }
    }

    pub struct GeminiProvider;
    impl GeminiProvider {
        pub fn new(_key: String, _model: String) -> Self { Self }
    }

    pub struct MinimaxProvider;
    impl MinimaxProvider {
        pub fn new(_key: String, _group_id: String, _model: String) -> Self { Self }
    }
}
