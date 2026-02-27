pub mod prelude {
    pub use crate::decision::*;
    pub use crate::mcp::*;
    pub use crate::agent::*;
    pub use async_trait::async_trait;
}

pub mod decision {
    use serde_json::Value;
    use anyhow::Result;
    use async_trait::async_trait;

    pub struct DecisionContext {
        pub summary: String,
    }

    impl DecisionContext {
        pub fn new(kind: &str) -> Self {
            Self { summary: String::new() }
        }
        pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
            self.summary = summary.into();
            self
        }
        pub fn with_metadata(mut self, _key: &str, _value: String) -> Self {
            self
        }
        pub fn with_data(mut self, _data: Value) -> Self {
            self
        }
        pub fn with_data(mut self, _data: Value) -> Self {
            self
        }
    }

    pub struct Decision {
        pub action: String,
        pub reasoning: String,
        pub confidence: f32,
        pub parameters: Option<Value>,
        pub providers_used: Vec<String>,
    }

    pub struct DecisionEngine {}

    impl DecisionEngine {
        pub fn new() -> Self {
            Self {}
        }
        pub fn builder() -> EngineBuilder {
            EngineBuilder {}
        }
        pub async fn decide(&self, _ctx: &DecisionContext) -> Result<Decision> {
            Ok(Decision {
                action: "chat".to_string(),
                reasoning: "Mocked decision".to_string(),
                confidence: 1.0,
                parameters: None,
                providers_used: vec![],
            })
        }
    }

    pub struct EngineBuilder {}
    impl EngineBuilder {
        pub fn with_provider<P>(self, _provider: P) -> Self { self }
        pub fn build(self) -> DecisionEngine { DecisionEngine {} }
    }

    pub struct MinimaxProvider {}
    impl MinimaxProvider {
        pub fn new(_key: String, _group: String, _model: String) -> Self { Self {} }
    }

    pub struct GeminiProvider {}
    impl GeminiProvider {
        pub fn new(_key: String, _model: String) -> Self { Self {} }
    }
}

pub mod mcp {
    use serde_json::Value;
    use async_trait::async_trait;
    use anyhow::Result;

    pub struct ToolRegistry {}
    impl ToolRegistry {
        pub fn new() -> Self { Self {} }
        pub async fn register_tool<T: Tool + 'static>(&self, _tool: T) {}
        pub async fn call(&self, _name: &str, _ctx: &dyn ToolContext, _args: Value) -> Result<Value> {
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

    pub trait ToolContext: Send + Sync {}
    pub struct EmptyContext;
    impl ToolContext for EmptyContext {}
}

pub mod agent {
    use async_trait::async_trait;
    #[async_trait]
    pub trait Agent {
        type Input;
        fn name(&self) -> &str;
        async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()>;
    }
}
