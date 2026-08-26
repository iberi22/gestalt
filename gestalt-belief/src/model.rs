//! Belief graph domain model: claims and typed edges.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A belief claim node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claim {
    /// Unique identifier for the claim.
    pub id: String,
    /// Natural language statement of the belief.
    pub statement: String,
    /// Confidence in [0.0, 1.0].
    pub confidence: f64,
    /// Last update timestamp (UTC).
    pub updated_at: DateTime<Utc>,
}

impl Claim {
    /// Create a new claim with current timestamp.
    pub fn new(id: impl Into<String>, statement: impl Into<String>, confidence: f64) -> Self {
        Self {
            id: id.into(),
            statement: statement.into(),
            confidence: confidence.clamp(0.0, 1.0),
            updated_at: Utc::now(),
        }
    }

    /// Create with explicit timestamp (for deterministic tests).
    pub fn with_updated_at(mut self, ts: DateTime<Utc>) -> Self {
        self.updated_at = ts;
        self
    }
}

/// Kind of directed edge between two claims.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// A supports B (A provides evidence for B).
    #[serde(rename = "supports")]
    Supports,
    /// A contradicts B.
    #[serde(rename = "contradicts")]
    Contradicts,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeKind::Supports => write!(f, "supports"),
            EdgeKind::Contradicts => write!(f, "contradicts"),
        }
    }
}

impl std::str::FromStr for EdgeKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "supports" => Ok(EdgeKind::Supports),
            "contradicts" => Ok(EdgeKind::Contradicts),
            other => Err(format!("Unknown EdgeKind: {other}")),
        }
    }
}

/// Weighted directed edge payload stored in the petgraph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeliefEdge {
    /// Edge semantics.
    pub kind: EdgeKind,
    /// Edge weight (typically 0..1, but not strictly clamped).
    pub weight: f64,
}

impl BeliefEdge {
    /// Create a new edge.
    pub fn new(kind: EdgeKind, weight: f64) -> Self {
        Self { kind, weight }
    }
}

/// Pure deterministic decay function.
///
/// Reduces confidence by half-life: `decayed = confidence * 0.5^(elapsed_days / half_life_days)`.
///
/// - If `half_life_days <= 0` the confidence is returned unchanged (no decay).
/// - If `elapsed_days <= 0` the confidence is returned unchanged (future/no time passed).
/// - Result is clamped to `[0.0, 1.0]`.
pub fn decay_confidence(confidence: f64, elapsed_days: f64, half_life_days: f64) -> f64 {
    if half_life_days <= 0.0 || elapsed_days <= 0.0 {
        return confidence.clamp(0.0, 1.0);
    }
    let c = confidence.clamp(0.0, 1.0);
    (c * 0.5_f64.powf(elapsed_days / half_life_days)).clamp(0.0, 1.0)
}

/// Compute elapsed days between two instants as fractional days.
pub fn elapsed_days(from: DateTime<Utc>, to: DateTime<Utc>) -> f64 {
    let dur = to.signed_duration_since(from);
    dur.num_milliseconds() as f64 / 86_400_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed(s: &str) -> DateTime<Utc> {
        s.parse::<DateTime<Utc>>().unwrap()
    }

    #[test]
    fn test_decay_halves() {
        // After exactly one half-life, confidence should halve.
        let c = decay_confidence(0.8, 7.0, 7.0);
        assert!((c - 0.4).abs() < 1e-9, "expected 0.4 got {c}");
    }

    #[test]
    fn test_decay_two_half_lives() {
        // After two half-lives, quarter.
        let c = decay_confidence(1.0, 14.0, 7.0);
        assert!((c - 0.25).abs() < 1e-9, "expected 0.25 got {c}");
    }

    #[test]
    fn test_decay_zero_elapsed() {
        let c = decay_confidence(0.6, 0.0, 7.0);
        assert!((c - 0.6).abs() < 1e-9);
    }

    #[test]
    fn test_decay_zero_half_life_noop() {
        let c = decay_confidence(0.6, 10.0, 0.0);
        assert!((c - 0.6).abs() < 1e-9);
    }

    #[test]
    fn test_elapsed_days_fixed() {
        let a = fixed("2026-01-01T00:00:00Z");
        let b = fixed("2026-01-08T00:00:00Z");
        assert!((elapsed_days(a, b) - 7.0).abs() < 1e-9);
        // fractional
        let c = fixed("2026-01-01T12:00:00Z");
        assert!((elapsed_days(a, c) - 0.5).abs() < 1e-9);
        let _ = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    }

    #[test]
    fn test_edge_kind_roundtrip() {
        for k in [EdgeKind::Supports, EdgeKind::Contradicts] {
            let s = k.to_string();
            let parsed: EdgeKind = s.parse().unwrap();
            assert_eq!(k, parsed);
        }
    }
}
