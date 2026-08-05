//! Belief Graph — conceptual graph used by Gestalt's reasoning layers.
//!
//! The `BeliefGraph` stores structured beliefs as a directed graph of
//! concept nodes connected by typed, weighted edges. It supports:
//!
//! - **Triple-based beliefs** — `(subject, predicate, object)` with configurable confidence
//! - **Graph traversal** — BFS, shortest-path, highest-confidence path (Dijkstra-like)
//! - **Keyword search** — find edges matching subject / predicate / object
//! - **Grounding validation** — check whether documents are supported by the graph
//!
//! This module is ported from Xavier's belief graph architecture and adapted
//! for Gestalt's hexagonal layout (ports & adapters).  The graph is
//! thread-safe via `RwLock` internally; an `Arc<tokio::sync::RwLock<BeliefGraph>>`
//! alias (`SharedBeliefGraph`) is provided for async ownership patterns.

use crate::domain::belief::{Belief, BeliefEdge, BeliefNode};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::info;

// ---------------------------------------------------------------------------
// Normalisation helpers (no external `qmd` dependency)
// ---------------------------------------------------------------------------

/// Simple concept normalisation: lowercase, trim, replace inner whitespace runs
/// with a single underscore, strip leading/trailing non-alphanumeric.
fn normalize_concept(raw: &str) -> String {
    let cleaned: String = raw
        .to_lowercase()
        .trim()
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    // Strip leading / trailing non-alphanumeric characters
    let trimmed = cleaned
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_string();

    if trimmed.is_empty() {
        "unknown".into()
    } else {
        trimmed
    }
}

// ---------------------------------------------------------------------------
// Inline confidence evaluator
// ---------------------------------------------------------------------------

/// A simple confidence-scoring function.  Higher scores for known provenance
/// sources, lower for inferred / unknown sources.
fn evaluate_confidence(source_type: &str, _relation_type: &str) -> f32 {
    match source_type {
        "user" | "system" | "session" | "observation" => 0.85,
        "inference" => 0.60,
        "synthesis" | "aggregation" => 0.70,
        "reflection" => 0.55,
        _ => 0.50,
    }
}

/// Detect whether two edges contradict one-another by comparing their source,
/// target, and relation type to known lexical contradiction pairs.
fn check_contradiction(a: &BeliefEdge, b: &BeliefEdge) -> Option<String> {
    // same source + target, opposite polarity
    if a.source == b.source && a.target == b.target {
        let pairs = [
            ("is_a", "is_not_a"),
            ("supports", "contradicts"),
            ("likes", "dislikes"),
            ("agrees", "disagrees"),
            ("increases", "decreases"),
            ("enables", "blocks"),
            ("requires", "bypasses"),
            ("is", "is_not"),
        ];
        for (x, y) in &pairs {
            if (a.relation_type == *x && b.relation_type == *y)
                || (a.relation_type == *y && b.relation_type == *x)
            {
                return Some(b.id.clone());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// BeliefGraph
// ---------------------------------------------------------------------------

/// Thread-safe belief graph exposing both synchronous helpers and async
/// convenience methods.
///
/// **Internal locking:** nodes / edges / adjacency are each guarded by a
/// `std::sync::RwLock`, so short reads do not block the Tokio runtime.
/// Long-running graph algorithms acquire a full snapshot under a single read.
#[derive(Debug)]
pub struct BeliefGraph {
    nodes: RwLock<HashMap<String, BeliefNode>>,
    edges: RwLock<Vec<BeliefEdge>>,
    adjacency: RwLock<HashMap<String, HashSet<String>>>,
}

impl BeliefGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adjacency: RwLock::new(HashMap::new()),
        }
    }

    // -----------------------------------------------------------------------
    // Node operations
    // -----------------------------------------------------------------------

    /// Ensure a node exists for `concept` with the given `confidence`.
    ///
    /// If the node already exists (by normalised concept) the call is a no-op.
    pub fn add_node(&self, concept: &str, confidence: f32) {
        let concept_norm = normalize_concept(concept);

        if self.get_node(&concept_norm).is_some() {
            return;
        }

        let node = BeliefNode::new(&concept_norm, confidence, None);

        self.nodes
            .write()
            .expect("belief_graph: nodes write lock poisoned")
            .insert(node.id.clone(), node);

        self.adjacency
            .write()
            .expect("belief_graph: adjacency write lock poisoned")
            .entry(concept_norm.clone())
            .or_default();

        info!("Added node: {}", concept_norm);
    }

    /// Return the first node matching the normalised `concept`, or `None`.
    pub fn get_node(&self, concept: &str) -> Option<BeliefNode> {
        let concept_norm = normalize_concept(concept);
        self.nodes
            .read()
            .expect("belief_graph: nodes read lock poisoned")
            .values()
            .find(|n| n.concept == concept_norm)
            .cloned()
    }

    /// Return a copy of every node.
    pub fn list_nodes(&self) -> Vec<BeliefNode> {
        self.nodes
            .read()
            .expect("belief_graph: nodes read lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    // -----------------------------------------------------------------------
    // Edge / relation operations
    // -----------------------------------------------------------------------

    /// Add a relation (edge) between two concepts.
    ///
    /// Auto-creates nodes for `source` and `target` if they do not exist yet.
    /// Detects contradictions against existing edges.
    ///
    /// When `confidence_override` is `Some(value)` that value is used instead
    /// of the inline evaluator — useful when propagating an explicit
    /// belief-level confidence score.
    pub async fn add_relation(
        &self,
        source: &str,
        target: &str,
        relation_type: &str,
        provenance_id: Option<String>,
        source_type: Option<&str>,
        confidence_override: Option<f32>,
    ) -> Result<()> {
        let source_norm = normalize_concept(source);
        let target_norm = normalize_concept(target);
        let pid = provenance_id.unwrap_or_else(|| "unknown".to_string());
        let confidence = confidence_override
            .unwrap_or_else(|| evaluate_confidence(source_type.unwrap_or("unknown"), relation_type));

        // Ensure both endpoints exist
        if self.get_node(&source_norm).is_none() {
            self.add_node(&source_norm, confidence);
        }
        if self.get_node(&target_norm).is_none() {
            self.add_node(&target_norm, confidence);
        }

        let mut edge = BeliefEdge::new(&source_norm, &target_norm, relation_type, confidence, &pid);

        if source_type == Some("inference") {
            edge.is_inferred = true;
        }

        // Detect contradiction
        let edges = self.get_edges();
        for existing in &edges {
            if let Some(c_id) = check_contradiction(&edge, existing) {
                edge.contradicts_edge_id = Some(c_id);
                info!(
                    "Contradiction detected for {} → {} ({})",
                    source_norm, target_norm, edge.relation_type
                );
                break;
            }
        }

        self.edges
            .write()
            .expect("belief_graph: edges write lock poisoned")
            .push(edge);

        self.adjacency
            .write()
            .expect("belief_graph: adjacency write lock poisoned")
            .entry(source_norm.clone())
            .or_default()
            .insert(target_norm.clone());

        info!(
            "Added relation: {} → {} ({}) [confidence: {}]",
            source_norm, target_norm, relation_type, confidence
        );
        Ok(())
    }

    /// Convenience: add an edge without provenance tracking.
    pub async fn add_edge(&self, from: &str, to: &str, relation: &str) {
        let _ = self
            .add_relation(from, to, relation, None, None, None)
            .await;
    }

    /// Add a belief triple as an edge with provenance.
    pub async fn add_belief(
        &self,
        belief: &Belief,
        source_memory_id: Option<String>,
    ) -> Result<()> {
        let confidence_score = belief.confidence.score();

        if self.get_node(&belief.subject).is_none() {
            self.add_node(&belief.subject, confidence_score);
        }
        if self.get_node(&belief.object).is_none() {
            self.add_node(&belief.object, confidence_score);
        }

        self.add_relation(
            &belief.subject,
            &belief.object,
            &belief.predicate,
            source_memory_id,
            None,
            Some(confidence_score),
        )
        .await
    }

    /// Return a copy of every edge.
    pub fn get_edges(&self) -> Vec<BeliefEdge> {
        self.edges
            .read()
            .expect("belief_graph: edges write lock poisoned")
            .clone()
    }

    /// Async convenience — same as `get_edges`.
    pub async fn get_edges_async(&self) -> Vec<BeliefEdge> {
        self.get_edges()
    }

    /// Alias for `get_edges`.
    pub fn get_relations(&self) -> Vec<BeliefEdge> {
        self.get_edges()
    }

    /// Replace all relations atomically, rebuilding node and adjacency state.
    pub fn replace_relations(&self, edges: Vec<BeliefEdge>) {
        let mut nodes = HashMap::new();
        let mut adjacency = HashMap::<String, HashSet<String>>::new();

        for edge in &edges {
            let sn = normalize_concept(&edge.source);
            let tn = normalize_concept(&edge.target);

            nodes
                .entry(sn.clone())
                .or_insert(BeliefNode::new(&sn, edge.confidence_score, None));
            nodes
                .entry(tn.clone())
                .or_insert(BeliefNode::new(&tn, edge.confidence_score, None));

            adjacency.entry(sn).or_default().insert(tn);
        }

        *self
            .nodes
            .write()
            .expect("belief_graph: nodes write lock poisoned") = nodes;
        *self
            .adjacency
            .write()
            .expect("belief_graph: adjacency write lock poisoned") = adjacency;
        *self
            .edges
            .write()
            .expect("belief_graph: edges write lock poisoned") = edges;
    }

    // -----------------------------------------------------------------------
    // Query / search
    // -----------------------------------------------------------------------

    /// Return the set of concepts directly connected from `concept`.
    pub fn get_related(&self, concept: &str) -> Vec<String> {
        let concept_norm = normalize_concept(concept);
        self.adjacency
            .read()
            .expect("belief_graph: adjacency read lock poisoned")
            .get(&concept_norm)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Search edges by keyword matching against source, target, and
    /// relation_type.  Results are sorted by descending confidence.
    pub async fn search(&self, query: &str) -> Vec<BeliefEdge> {
        let query_lower = query.to_lowercase();
        let words: Vec<_> = query_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .collect();

        if words.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<_> = self
            .get_edges()
            .into_iter()
            .filter(|edge| {
                let s = edge.source.to_lowercase();
                let t = edge.target.to_lowercase();
                let r = edge.relation_type.to_lowercase();
                words
                    .iter()
                    .any(|w| s.contains(w) || t.contains(w) || r.contains(w))
            })
            .collect();

        results.sort_by(|a, b| {
            b.confidence_score
                .partial_cmp(&a.confidence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Breadth-first traversal from `start`, returning reached nodes (excluding
    /// the start node itself).
    pub async fn bfs(&self, start: &str) -> Vec<String> {
        let start_norm = normalize_concept(start);
        let adjacency = self
            .adjacency
            .read()
            .expect("belief_graph: adjacency read lock poisoned")
            .clone();

        let mut visited = HashSet::new();
        let mut queue = VecDeque::from([start_norm]);
        let mut ordered = Vec::new();

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if current != normalize_concept(start) {
                ordered.push(current.clone());
            }
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
        ordered
    }

    /// Dijkstra-like search for the highest-confidence path between two concepts.
    ///
    /// Edge cost is defined as `1.0 - confidence_score` (higher confidence =
    /// lower cost).  Returns the edges along the optimal path in order.
    pub async fn find_highest_confidence_path(&self, start: &str, end: &str) -> Vec<BeliefEdge> {
        let start_norm = normalize_concept(start);
        let end_norm = normalize_concept(end);

        let edges = self.get_edges();
        let mut distances = HashMap::new();
        let mut previous = HashMap::new();
        let mut queue: HashSet<String> = HashSet::new();

        distances.insert(start_norm.clone(), 0.0f32);
        queue.insert(start_norm.clone());

        while !queue.is_empty() {
            let current = queue
                .iter()
                .min_by(|a, b| {
                    let da = distances.get(*a).copied().unwrap_or(f32::INFINITY);
                    let db = distances.get(*b).copied().unwrap_or(f32::INFINITY);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned()
                .expect("belief_graph: find_highest_confidence_path had empty queue");

            queue.remove(&current);

            if current == end_norm {
                break;
            }

            for edge in edges.iter().filter(|e| e.source == current) {
                let alt = distances.get(&current).copied().unwrap_or(f32::INFINITY)
                    + (1.0 - edge.confidence_score);
                if alt < *distances.get(&edge.target).unwrap_or(&f32::INFINITY) {
                    distances.insert(edge.target.clone(), alt);
                    previous.insert(edge.target.clone(), edge.clone());
                    queue.insert(edge.target.clone());
                }
            }
        }

        let mut path = Vec::new();
        let mut curr = end_norm.clone();
        while let Some(edge) = previous.get(&curr) {
            path.push(edge.clone());
            curr = edge.source.clone();
        }
        path.reverse();
        path
    }

    // -----------------------------------------------------------------------
    // Grounding
    // -----------------------------------------------------------------------

    /// Check whether any edge was created with the given `memory_id` as its
    /// provenance — i.e. whether a memory document directly grounded a belief.
    pub async fn has_supporting_beliefs(&self, memory_id: &str) -> bool {
        self.get_edges()
            .iter()
            .any(|e| e.provenance_id == memory_id)
    }

    /// Validate whether a set of documents are grounded in the belief graph.
    ///
    /// Returns `(id_or_path, is_grounded, explanation)` for each document.
    ///
    /// Grounding is established when:
    /// 1. The document's `id` matches an edge's `provenance_id` (direct), **or**
    /// 2. The document's content contains keywords matching any graph node with
    ///    ≥ `min_confidence` confidence.
    pub async fn validate_grounding(
        &self,
        documents: &[MemoryDocument],
        min_confidence: f32,
    ) -> Vec<(String, bool, String)> {
        let edges = self.get_edges();
        let nodes = self.list_nodes();

        let eligible_concepts: Vec<&str> = nodes
            .iter()
            .filter(|n| n.confidence >= min_confidence)
            .map(|n| n.concept.as_str())
            .collect();

        let mut results = Vec::new();
        for doc in documents {
            let memory_id = doc.id.clone().unwrap_or_else(|| doc.path.clone());

            // Direct provenance match
            if edges.iter().any(|e| e.provenance_id == memory_id) {
                results.push((memory_id, true, "Directly grounded in belief graph".into()));
                continue;
            }

            // Semantic keyword match
            let content_lower = doc.content.to_lowercase();
            let matched: Vec<&str> = eligible_concepts
                .iter()
                .filter(|c| content_lower.contains(*c))
                .copied()
                .collect();

            if !matched.is_empty() {
                results.push((
                    memory_id,
                    true,
                    format!("Semantically grounded through concepts: {:?}", matched),
                ));
            } else {
                results.push((
                    memory_id,
                    false,
                    "No supporting beliefs or nodes found in graph".into(),
                ));
            }
        }
        results
    }

    /// Return a summary of the graph's current size.
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.nodes.read().expect("poisoned").len(),
            edge_count: self.edges.read().expect("poisoned").len(),
        }
    }
}

impl Default for BeliefGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A lightweight document descriptor used for grounding validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDocument {
    pub id: Option<String>,
    pub path: String,
    pub content: String,
}

impl Default for MemoryDocument {
    fn default() -> Self {
        Self {
            id: None,
            path: String::new(),
            content: String::new(),
        }
    }
}

/// Numeric summary of the graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
}

/// Thread-safe handle for sharing a `BeliefGraph` across async boundaries.
pub type SharedBeliefGraph = Arc<AsyncRwLock<BeliefGraph>>;

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::belief::Confidence;

    // -----------------------------------------------------------------------
    // Core operations
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_add_node() {
        let graph = BeliefGraph::new();
        graph.add_node("Rust Language", 0.9);
        assert_eq!(graph.list_nodes().len(), 1);
        assert!(graph.get_node("rust_language").is_some());
    }

    #[tokio::test]
    async fn test_add_node_idempotent() {
        let graph = BeliefGraph::new();
        graph.add_node("Rust", 0.9);
        graph.add_node("rust", 0.5); // same normalised form
        assert_eq!(graph.list_nodes().len(), 1);
    }

    #[tokio::test]
    async fn test_add_edge() {
        let graph = BeliefGraph::new();
        graph.add_edge("Xavier", "Gestalt", "depends_on").await;
        let edges = graph.get_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, "xavier");
        assert_eq!(edges[0].target, "gestalt");
    }

    #[tokio::test]
    async fn test_add_relation_auto_creates_nodes() {
        let graph = BeliefGraph::new();
        graph
            .add_relation("Alice", "Bob", "knows", None, None, None)
            .await
            .unwrap();
        assert_eq!(graph.list_nodes().len(), 2);
        assert!(graph.get_node("Alice").is_some());
        assert!(graph.get_node("Bob").is_some());
    }

    #[tokio::test]
    async fn test_add_belief() {
        let graph = BeliefGraph::new();
        let belief = Belief::new("Rust", "is_fast", "C++", Confidence::High);
        graph
            .add_belief(&belief, Some("session-1".into()))
            .await
            .unwrap();

        let edges = graph.get_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, "rust");
        // "C++" normalizes to "c" (non-alphanumeric trimmed)
        assert_eq!(edges[0].target, "c");
        assert_eq!(edges[0].relation_type, "is_fast");
        assert!((edges[0].confidence_score - 0.9).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Query
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_related() {
        let graph = BeliefGraph::new();
        graph.add_edge("A", "B", "connects").await;
        graph.add_edge("A", "C", "connects").await;
        let related = graph.get_related("A");
        assert_eq!(related.len(), 2);
        assert!(related.contains(&"b".to_string()));
        assert!(related.contains(&"c".to_string()));
    }

    #[tokio::test]
    async fn test_search() {
        let graph = BeliefGraph::new();
        graph.add_edge("Rust", "C++", "faster_than").await;
        graph.add_edge("Python", "Java", "similar_to").await;

        let results = graph.search("rust").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "rust");

        let all = graph.search("rust java python").await;
        assert_eq!(all.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Traversal
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_bfs() {
        let graph = BeliefGraph::new();
        graph.add_edge("A", "B", "link").await;
        graph.add_edge("A", "C", "link").await;
        graph.add_edge("B", "D", "link").await;

        let order = graph.bfs("a").await;
        // Should contain B, C, D in BFS order (might be B, C, D or C, B, D)
        assert_eq!(order.len(), 3);
    }

    #[tokio::test]
    async fn test_highest_confidence_path() {
        let graph = BeliefGraph::new();

        // Create two paths between A and D
        graph
            .add_relation("A", "B", "step", Some("m1".into()), Some("user"), None)
            .await
            .unwrap(); // conf 0.85
        graph
            .add_relation("A", "C", "step", Some("m2".into()), Some("user"), None)
            .await
            .unwrap(); // conf 0.85
        graph
            .add_relation("B", "D", "step", Some("m3".into()), Some("inference"), None)
            .await
            .unwrap(); // conf 0.60 → A→B→D = 0.85+0.60 = 1.45, but cost = (1-0.85)+(1-0.60)=0.55
        graph
            .add_relation("C", "D", "step", Some("m4".into()), Some("user"), None)
            .await
            .unwrap(); // conf 0.85 → A→C→D = 0.85+0.85, cost = (1-0.85)+(1-0.85)=0.30

        let path = graph.find_highest_confidence_path("A", "D").await;
        // A→C→D should be preferred (lower cost)
        assert_eq!(path.len(), 2);
        // First edge should be A→C (lower cost path A→C→D)
        assert_eq!(path[0].source, "a");
        assert_eq!(path[0].target, "c");
    }

    // -----------------------------------------------------------------------
    // Contradiction
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_contradiction_detection() {
        let graph = BeliefGraph::new();
        graph
            .add_relation("Alice", "Bob", "likes", Some("s1".into()), Some("user"), None)
            .await
            .unwrap();
        graph
            .add_relation("Alice", "Bob", "dislikes", Some("s2".into()), Some("user"), None)
            .await
            .unwrap();

        let edges = graph.get_edges();
        assert_eq!(edges.len(), 2);
        // The second edge should have contradicts_edge_id set
        let second = edges
            .iter()
            .find(|e| e.relation_type == "dislikes")
            .unwrap();
        assert!(second.contradicts_edge_id.is_some());
    }

    // -----------------------------------------------------------------------
    // Grounding validation
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_grounding_direct() {
        let graph = BeliefGraph::new();
        graph
            .add_relation("Xavier", "Memory", "is_a", Some("mem-1".into()), None, None)
            .await
            .unwrap();

        let docs = vec![MemoryDocument {
            id: Some("mem-1".into()),
            path: "/tmp/doc.md".into(),
            content: "Something about Xavier".into(),
        }];

        let results = graph.validate_grounding(&docs, 0.5).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].1);
        assert!(results[0].2.contains("Directly grounded"));
    }

    #[tokio::test]
    async fn test_validate_grounding_semantic() {
        let graph = BeliefGraph::new();
        graph.add_node("Xavier", 0.9);
        graph.add_node("Rust", 0.4);

        let docs = vec![MemoryDocument {
            id: Some("doc-1".into()),
            path: "/tmp/xavier.md".into(),
            content: "Xavier is written in Rust".into(),
        }];

        // min_confidence = 0.5 → only Xavier matches
        let results = graph.validate_grounding(&docs, 0.5).await;
        assert!(results[0].1);
        assert!(results[0].2.contains("xavier"));
        assert!(!results[0].2.contains("rust"));

        // min_confidence = 0.3 → both match
        let results = graph.validate_grounding(&docs, 0.3).await;
        assert!(results[0].2.contains("xavier"));
        assert!(results[0].2.contains("rust"));
    }

    #[tokio::test]
    async fn test_validate_grounding_no_match() {
        let graph = BeliefGraph::new();
        graph.add_node("Xavier", 0.9);

        let docs = vec![MemoryDocument {
            id: Some("doc-1".into()),
            path: "/tmp/other.md".into(),
            content: "Something unrelated".into(),
        }];

        let results = graph.validate_grounding(&docs, 0.5).await;
        assert!(!results[0].1);
    }

    // -----------------------------------------------------------------------
    // Normalisation stability
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_node_id_normalization() {
        let graph = BeliefGraph::new();
        graph.add_node("My Concept", 0.9);
        graph.add_node("my_concept", 0.8);

        let nodes = graph.list_nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].concept, "my_concept");
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_stats() {
        let graph = BeliefGraph::new();
        graph.add_node("A", 0.5);
        graph.add_node("B", 0.5);
        graph.add_edge("A", "B", "link").await;
        let s = graph.stats();
        assert_eq!(s.node_count, 2);
        assert_eq!(s.edge_count, 1);
    }

    // -----------------------------------------------------------------------
    // Replace relations
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_replace_relations() {
        let graph = BeliefGraph::new();
        graph.add_edge("A", "B", "link").await;

        let new_edges = vec![BeliefEdge::new("C", "D", "new_link", 0.5, "replacement")];
        graph.replace_relations(new_edges);

        assert_eq!(graph.get_edges().len(), 1);
        // replace_relations normalizes nodes but keeps edge fields as-is
        assert_eq!(graph.get_edges()[0].source, "C");
        assert!(graph.get_node("c").is_some());
        assert!(graph.get_node("d").is_some());
        // Old nodes should be gone
        assert!(graph.get_node("a").is_none());
    }
}
