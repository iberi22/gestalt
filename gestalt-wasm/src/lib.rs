use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export MemoryNode, MemoryEdge, GraphOps, MemorySync, EventBus from gestalt-proto
pub use gestalt_proto::memory::{GraphOps, MemoryEdge, MemoryNode, MemorySync};
pub use gestalt_proto::event::EventBus;

pub mod git;
pub mod state;

#[wasm_bindgen]
pub struct WasmGraph {
    nodes: Vec<MemoryNode>,
    edges: Vec<MemoryEdge>,
}

#[wasm_bindgen]
impl WasmGraph {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmGraph {
        WasmGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: MemoryNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: MemoryEdge) {
        self.edges.push(edge);
    }

    pub fn get_nodes(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.nodes).unwrap_or(JsValue::NULL)
    }

    pub fn get_edges(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.edges).unwrap_or(JsValue::NULL)
    }
}

impl GraphOps for WasmGraph {
    fn add_node(&mut self, node: MemoryNode) {
        self.add_node(node);
    }

    fn add_edge(&mut self, edge: MemoryEdge) {
        self.add_edge(edge);
    }

    fn get_nodes(&self) -> Vec<MemoryNode> {
        self.nodes.clone()
    }

    fn get_edges(&self) -> Vec<MemoryEdge> {
        self.edges.clone()
    }
}

#[wasm_bindgen]
pub struct MemorySyncWrapper {}

#[wasm_bindgen]
impl MemorySyncWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MemorySyncWrapper {
        MemorySyncWrapper {}
    }

    pub fn sync(&mut self) -> Result<(), JsValue> {
        Ok(())
    }
}

impl MemorySync for MemorySyncWrapper {
    fn sync(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[wasm_bindgen]
pub struct WasmEventBus {
    callback: Option<js_sys::Function>,
}

#[wasm_bindgen]
impl WasmEventBus {
    #[wasm_bindgen(constructor)]
    pub fn new(callback: Option<js_sys::Function>) -> WasmEventBus {
        WasmEventBus { callback }
    }

    pub fn publish(&self, event: String) {
        if let Some(ref cb) = self.callback {
            let this = JsValue::NULL;
            let val = JsValue::from_str(&event);
            let _ = cb.call1(&this, &val);
        }
    }
}

impl EventBus for WasmEventBus {
    fn publish(&self, event: String) {
        self.publish(event);
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentSpec {
    #[wasm_bindgen(getter_with_clone)]
    pub id: String,
    #[wasm_bindgen(getter_with_clone)]
    pub command: String,
    #[wasm_bindgen(getter_with_clone)]
    pub args: Vec<String>,
}

#[wasm_bindgen]
impl AgentSpec {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, command: String, args: Vec<String>) -> AgentSpec {
        AgentSpec { id, command, args }
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RunSpec {
    #[wasm_bindgen(getter_with_clone)]
    pub base_ref: String,
    #[wasm_bindgen(getter_with_clone)]
    pub task: String,
    agents: Vec<AgentSpec>,
    pub max_parallel: usize,
    pub timeout: f64,
    pub push: bool,
    #[wasm_bindgen(getter_with_clone)]
    pub integration_branch: Option<String>,
}

#[wasm_bindgen]
impl RunSpec {
    #[wasm_bindgen(constructor)]
    pub fn new(
        base_ref: String,
        task: String,
        agents: JsValue,
        max_parallel: usize,
        timeout: f64,
        push: bool,
        integration_branch: Option<String>,
    ) -> Result<RunSpec, JsValue> {
        let agents: Vec<AgentSpec> = serde_wasm_bindgen::from_value(agents)?;
        Ok(RunSpec {
            base_ref,
            task,
            agents,
            max_parallel,
            timeout,
            push,
            integration_branch,
        })
    }

    #[wasm_bindgen(getter)]
    pub fn agents(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.agents).unwrap_or(JsValue::NULL)
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentResult {
    #[wasm_bindgen(getter_with_clone)]
    pub agent_id: String,
    #[wasm_bindgen(getter_with_clone)]
    pub output: Option<String>,
    #[wasm_bindgen(getter_with_clone)]
    pub error: Option<String>,
    #[wasm_bindgen(getter_with_clone)]
    pub branch: Option<String>,
    #[wasm_bindgen(getter_with_clone)]
    pub changed_files: Vec<String>,
    pub duration_ms: f64,
}

#[wasm_bindgen]
impl AgentResult {
    #[wasm_bindgen(constructor)]
    pub fn new(
        agent_id: String,
        output: Option<String>,
        error: Option<String>,
        branch: Option<String>,
        changed_files: Vec<String>,
        duration_ms: f64,
    ) -> AgentResult {
        AgentResult {
            agent_id,
            output,
            error,
            branch,
            changed_files,
            duration_ms,
        }
    }
}

#[wasm_bindgen]
pub fn init_gestalt() -> GestaltEngine {
    GestaltEngine {}
}

#[wasm_bindgen]
pub struct GestaltEngine {}

#[wasm_bindgen]
impl GestaltEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> GestaltEngine {
        GestaltEngine {}
    }

    pub fn execute_run_spec(&self, spec_val: JsValue) -> Result<RunReport, JsValue> {
        let spec: RunSpec = serde_wasm_bindgen::from_value(spec_val)
            .map_err(|e| JsValue::from_str(&format!("Invalid RunSpec: {}", e)))?;

        let mut agents_results = Vec::new();
        for agent in spec.agents {
            agents_results.push(AgentResult {
                agent_id: agent.id.clone(),
                output: Some(format!("Executed agent: {} with command: {}", agent.id, agent.command)),
                error: None,
                branch: Some(format!("feature/{}", agent.id)),
                changed_files: vec![format!("src/{}.rs", agent.id)],
                duration_ms: 120.0,
            });
        }

        Ok(RunReport {
            run_id: uuid::Uuid::new_v4().to_string(),
            task: spec.task,
            agents: agents_results,
            duration_ms: 150.0,
            merged_branches: vec!["main".to_string()],
            conflicts: Vec::new(),
            events_path: "/tmp/events".to_string(),
            success: true,
        })
    }

    pub fn subscribe_events(&self) -> EventStream {
        EventStream {
            events: vec![
                "Engine initialized".to_string(),
                "Execution started".to_string(),
                "Agents dispatched".to_string(),
                "Integration completed".to_string(),
            ],
            index: 0,
        }
    }
}

#[wasm_bindgen]
pub struct EventStream {
    events: Vec<String>,
    index: usize,
}

#[wasm_bindgen]
impl EventStream {
    #[wasm_bindgen(constructor)]
    pub fn new(events: JsValue) -> Result<EventStream, JsValue> {
        let events: Vec<String> = serde_wasm_bindgen::from_value(events)?;
        Ok(EventStream { events, index: 0 })
    }

    pub fn next(&mut self) -> Option<String> {
        if self.index < self.events.len() {
            let ev = self.events[self.index].clone();
            self.index += 1;
            Some(ev)
        } else {
            None
        }
    }
}

#[wasm_bindgen]
pub fn init_gestalt_engine() -> GestaltEngine {
    GestaltEngine::new()
}
