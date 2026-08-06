//! BeliefGraph core domain types and implementation.
//!
//! Defines the core data structures and graph logic for the belief graph:
//! - `BeliefNode` — a concept node representing a belief.
//! - `BeliefEdge` — a directed, weighted, typed connection between two nodes.
//! - `BeliefGraph` — the flat adjacency/collection-based belief graph.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A node representing a belief concept in the domain graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeliefNode {
    pub id: String,
    pub label: String,
    pub belief: f64, // ranges from 0.0 to 1.0 (0..1)
    pub updated_at: DateTime<Utc>,
}

/// A directed, typed edge connecting two `BeliefNode`s.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeliefEdge {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub relation: String,
}

/// The non-thread-safe domain representation of a Belief Graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BeliefGraph {
    pub nodes: HashMap<String, BeliefNode>,
    pub edges: Vec<BeliefEdge>,
}

impl BeliefGraph {
    /// Create a new empty `BeliefGraph`.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add a node to the graph. If it already exists, updates its label and belief.
    pub fn add_node(&mut self, id: impl Into<String>, label: impl Into<String>, belief: f64) {
        let id_str = id.into();
        let clamped_belief = belief.clamp(0.0, 1.0);
        let node = BeliefNode {
            id: id_str.clone(),
            label: label.into(),
            belief: clamped_belief,
            updated_at: Utc::now(),
        };
        self.nodes.insert(id_str, node);
    }

    /// Add an edge to the graph.
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>, weight: f64, relation: impl Into<String>) {
        let edge = BeliefEdge {
            from: from.into(),
            to: to.into(),
            weight,
            relation: relation.into(),
        };
        self.edges.push(edge);
    }

    /// Retrieve a reference to a node by its ID.
    pub fn get_node(&self, id: &str) -> Option<&BeliefNode> {
        self.nodes.get(id)
    }

    /// Update the belief score of a node if it exists, returning whether the update succeeded.
    pub fn update_belief(&mut self, id: &str, belief: f64) -> bool {
        if let Some(node) = self.nodes.get_mut(id) {
            node.belief = belief.clamp(0.0, 1.0);
            node.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Get a list of target node IDs adjacent to the given node ID.
    pub fn neighbors(&self, id: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .map(|e| e.to.clone())
            .collect()
    }

    /// Export the graph representation as a JSON `Value`.
    pub fn export_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Serialize the current graph state to a JSON string.
    pub fn save_to_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Load graph state from a JSON string.
    pub fn load_from_string(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ---------------------------------------------------------------------------
// Conversion Traits for Integration with PR #503 / existing types
// ---------------------------------------------------------------------------

impl From<crate::domain::belief::BeliefNode> for BeliefNode {
    fn from(n: crate::domain::belief::BeliefNode) -> Self {
        Self {
            id: n.id,
            label: n.concept,
            belief: n.confidence as f64,
            updated_at: n.created_at,
        }
    }
}

impl From<BeliefNode> for crate::domain::belief::BeliefNode {
    fn from(n: BeliefNode) -> Self {
        Self {
            id: n.id,
            concept: n.label,
            confidence: n.belief as f32,
            language_family: None,
            created_at: n.updated_at,
        }
    }
}

impl From<crate::domain::belief::BeliefEdge> for BeliefEdge {
    fn from(e: crate::domain::belief::BeliefEdge) -> Self {
        Self {
            from: e.source,
            to: e.target,
            weight: e.weight as f64,
            relation: e.relation_type,
        }
    }
}

impl From<BeliefEdge> for crate::domain::belief::BeliefEdge {
    fn from(e: BeliefEdge) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source: e.from,
            target: e.to,
            relation_type: e.relation,
            weight: e.weight as f32,
            confidence_score: e.weight as f32,
            provenance_id: "unknown".to_string(),
            contradicts_edge_id: None,
            is_inferred: false,
            source_language: None,
            target_language: None,
            created_at: now,
            updated_at: now,
        }
    }
}

impl From<crate::context::belief_graph::PersistedBeliefGraph> for BeliefGraph {
    fn from(p: crate::context::belief_graph::PersistedBeliefGraph) -> Self {
        let nodes = p.nodes.into_iter().map(|(k, v)| (k, BeliefNode::from(v))).collect();
        let edges = p.edges.into_iter().map(BeliefEdge::from).collect();
        Self { nodes, edges }
    }
}

impl From<BeliefGraph> for crate::context::belief_graph::PersistedBeliefGraph {
    fn from(g: BeliefGraph) -> Self {
        let nodes = g.nodes.into_iter().map(|(k, v)| (k, crate::domain::belief::BeliefNode::from(v))).collect();
        let edges = g.edges.into_iter().map(crate::domain::belief::BeliefEdge::from).collect();
        Self { nodes, edges }
    }
}
