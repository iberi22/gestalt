use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub mod prelude {
    pub use crate::{Tool, ToolContext, ToolRegistry};
    pub use async_trait::async_trait;
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn call(&self, ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value>;
}

pub trait ToolContext: Send + Sync {}

pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn register_tool<T: Tool + 'static>(&self, tool: T) {
        let mut tools = self.tools.write().await;
        tools.insert(tool.name().to_string(), Arc::new(tool));
    }
}
