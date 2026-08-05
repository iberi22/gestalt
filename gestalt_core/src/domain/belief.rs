//! Belief graph domain types for Gestalt.
//!
//! Defines the core data structures for the Gestalt belief system:
//! - `Confidence` — discrete confidence levels (High/Medium/Low)
//! - `Belief` — a triple (subject, predicate, object) with confidence
//! - `BeliefNode` — a concept node in the belief graph
//! - `BeliefEdge` — a directed, typed relation between two nodes
//!
//! Ported from Xavier's belief graph architecture and adapted for Gestalt's
//! hexagonal (ports & adapters) layout.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Confidence
// ---------------------------------------------------------------------------

/// Discrete confidence levels used when building beliefs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    /// Numeric score for the confidence level.
    ///
    /// | Level  | Score |
    /// |--------|-------|
    /// | High   | 0.90  |
    /// | Medium | 0.60  |
    /// | Low    | 0.30  |
    pub fn score(self) -> f32 {
        match self {
            Self::High => 0.9,
            Self::Medium => 0.6,
            Self::Low => 0.3,
        }
    }
}

// ---------------------------------------------------------------------------
// Belief (triple-based)
// ---------------------------------------------------------------------------

/// A declarative belief expressed as a subject–predicate–object triple.
///
/// # Examples
///
/// ```text
/// Belief { subject: "Alice", predicate: "knows", object: "Bob", confidence: High }
/// Belief { subject: "Rust", predicate: "is_fast", object: "C++", confidence: Medium }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Belief {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: Confidence,
}

impl Belief {
    pub fn new(
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        confidence: Confidence,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence,
        }
    }
}

// ---------------------------------------------------------------------------
// BeliefNode
// ---------------------------------------------------------------------------

/// A node in the belief graph representing a single concept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeliefNode {
    pub id: String,
    pub concept: String,
    pub confidence: f32,
    pub language_family: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl BeliefNode {
    /// Create a new `BeliefNode` with an auto-generated ULID.
    pub fn new(
        concept: impl Into<String>,
        confidence: f32,
        language_family: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            concept: concept.into(),
            confidence,
            language_family,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// BeliefEdge
// ---------------------------------------------------------------------------

/// A directed, typed edge connecting two `BeliefNode`s.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeliefEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub weight: f32,
    pub confidence_score: f32,
    pub provenance_id: String,
    pub contradicts_edge_id: Option<String>,
    pub is_inferred: bool,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BeliefEdge {
    /// Create a new `BeliefEdge` with an auto-generated ULID.
    pub fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        relation_type: impl Into<String>,
        confidence_score: f32,
        provenance_id: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            source: source.into(),
            target: target.into(),
            relation_type: relation_type.into(),
            weight: confidence_score,
            confidence_score,
            provenance_id: provenance_id.into(),
            contradicts_edge_id: None,
            is_inferred: false,
            source_language: None,
            target_language: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_score() {
        assert!((Confidence::High.score() - 0.9).abs() < f32::EPSILON);
        assert!((Confidence::Medium.score() - 0.6).abs() < f32::EPSILON);
        assert!((Confidence::Low.score() - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_belief_creation() {
        let b = Belief::new("Alice", "knows", "Bob", Confidence::High);
        assert_eq!(b.subject, "Alice");
        assert_eq!(b.predicate, "knows");
        assert_eq!(b.object, "Bob");
        assert_eq!(b.confidence, Confidence::High);
    }

    #[test]
    fn test_belief_node_auto_id() {
        let n1 = BeliefNode::new("Rust", 0.9, Some("Rust".into()));
        let n2 = BeliefNode::new("Rust", 0.9, Some("Rust".into()));
        assert_ne!(n1.id, n2.id);
        assert_eq!(n1.concept, "Rust");
    }

    #[test]
    fn test_belief_edge_creation() {
        let e = BeliefEdge::new("Alice", "Bob", "knows", 0.9, "session-1");
        assert_eq!(e.source, "Alice");
        assert_eq!(e.target, "Bob");
        assert_eq!(e.relation_type, "knows");
        assert!((e.confidence_score - 0.9).abs() < f32::EPSILON);
        assert!(!e.is_inferred);
        assert!(e.contradicts_edge_id.is_none());
    }

    #[test]
    fn test_belief_serialization_roundtrip() {
        let b = Belief::new("Xavier", "depends_on", "Gestalt", Confidence::Medium);
        let json = serde_json::to_string(&b).unwrap();
        let deserialized: Belief = serde_json::from_str(&json).unwrap();
        assert_eq!(b, deserialized);
    }
}
