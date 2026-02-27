pub mod prelude {
    pub use anyhow;
    pub use async_trait::async_trait;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;

    pub struct DecisionEngine;
    impl DecisionEngine {
        pub fn builder() -> DecisionEngineBuilder { DecisionEngineBuilder }
    }
    pub struct DecisionEngineBuilder;
    impl DecisionEngineBuilder {
        pub fn with_provider(self, _p: GeminiProvider) -> Self { self }
        pub fn build(self) -> DecisionEngine { DecisionEngine }
    }
    impl DecisionEngine {
        pub async fn decide(&self, _ctx: &DecisionContext) -> anyhow::Result<Decision> {
            Ok(Decision { action: "mock".to_string(), parameters: None })
        }
    }
    pub struct GeminiProvider;
    impl GeminiProvider {
        pub fn new(_key: String, _model: String) -> Self { GeminiProvider }
    }
    pub struct DecisionContext;
    impl DecisionContext {
        pub fn new(_q: &str) -> Self { DecisionContext }
        pub fn with_metadata(self, _k: &str, _v: String) -> Self { self }
    }
    pub struct Decision {
        pub action: String,
        pub parameters: Option<serde_json::Value>,
    }
    pub struct ToolRegistry;
    impl ToolRegistry {
        pub async fn call(&self, _name: &str, _ctx: &EmptyContext, _args: serde_json::Value) -> anyhow::Result<String> {
            Ok("mock result".to_string())
        }
    }
    pub struct EmptyContext;
    #[async_trait]
    pub trait Agent {
        type Input;
        fn name(&self) -> &str;
        async fn handle(&mut self, msg: Self::Input) -> anyhow::Result<()>;
    }
}
