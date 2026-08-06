use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryNode {
    #[wasm_bindgen(getter_with_clone)]
    pub id: String,
    #[wasm_bindgen(getter_with_clone)]
    pub label: String,
    #[wasm_bindgen(getter_with_clone)]
    pub properties: String,
}

#[wasm_bindgen]
impl MemoryNode {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, label: String, properties: String) -> Self {
        Self {
            id,
            label,
            properties,
        }
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryEdge {
    #[wasm_bindgen(getter_with_clone)]
    pub id: String,
    #[wasm_bindgen(getter_with_clone)]
    pub source: String,
    #[wasm_bindgen(getter_with_clone)]
    pub target: String,
    #[wasm_bindgen(getter_with_clone)]
    pub relation: String,
}

#[wasm_bindgen]
impl MemoryEdge {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, source: String, target: String, relation: String) -> Self {
        Self {
            id,
            source,
            target,
            relation,
        }
    }
}

pub trait GraphOps {
    fn add_node(&mut self, node: MemoryNode);
    fn add_edge(&mut self, edge: MemoryEdge);
    fn get_nodes(&self) -> Vec<MemoryNode>;
    fn get_edges(&self) -> Vec<MemoryEdge>;
}

pub trait MemorySync {
    fn sync(&mut self) -> Result<(), String>;
}

pub trait EventBus {
    fn publish(&self, event: String);
}
