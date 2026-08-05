use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use crate::context::ContextState;

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
                NodeResult::Continue(None) => {},
                NodeResult::Error(err) => {
                    state.set_value("error", Value::String(err));
                    if let Some(handler) = self.error_handler.clone() {
                        current = handler;
                    } else {
                        break;
                    }
                },
            }
        }

        Ok(state)
    }
}
