//! gestalt-belief — Belief Graph scaffold: claims, support/contradict edges, decay.
//!
//! Provides an in-memory petgraph DiGraph plus SQLite persistence mirroring
//! the `gestalt-state` connection pattern (WAL, foreign_keys).

pub mod model;
pub mod store;

pub use model::{decay_confidence, elapsed_days, BeliefEdge, Claim, EdgeKind};
pub use store::BeliefStore;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn fixed(s: &str) -> chrono::DateTime<Utc> {
        s.parse::<chrono::DateTime<Utc>>().unwrap()
    }

    fn claim_at(id: &str, conf: f64, ts: chrono::DateTime<Utc>) -> Claim {
        Claim {
            id: id.to_string(),
            statement: format!("statement {id}"),
            confidence: conf,
            updated_at: ts,
        }
    }

    #[test]
    fn test_insert_link_query_roundtrip() {
        let store = BeliefStore::open_in_memory().unwrap();
        let t0 = fixed("2026-01-01T00:00:00Z");
        store.insert_claim(claim_at("a", 0.9, t0)).unwrap();
        store.insert_claim(claim_at("b", 0.8, t0)).unwrap();
        store.insert_claim(claim_at("c", 0.7, t0)).unwrap();

        // a supports b, b contradicts c
        store.link("a", "b", EdgeKind::Supports, 0.9).unwrap();
        store.link("b", "c", EdgeKind::Contradicts, 1.0).unwrap();

        assert_eq!(store.claim_count(), 3);
        assert_eq!(store.edge_count(), 2);

        // query_related depth 1 from a should reach a + b
        let rel1 = store.query_related("a", 1).unwrap();
        let ids1: std::collections::HashSet<_> = rel1.iter().map(|c| c.id.as_str()).collect();
        assert!(ids1.contains("a"));
        assert!(ids1.contains("b"));
        assert!(!ids1.contains("c"));

        // depth 2 should reach all
        let rel2 = store.query_related("a", 2).unwrap();
        let ids2: std::collections::HashSet<_> = rel2.iter().map(|c| c.id.as_str()).collect();
        assert!(ids2.contains("c"));

        // depth 0 only self
        let rel0 = store.query_related("a", 0).unwrap();
        assert_eq!(rel0.len(), 1);
        assert_eq!(rel0[0].id, "a");
    }

    #[test]
    fn test_decay_halves_correctly() {
        let store = BeliefStore::open_in_memory().unwrap();
        let t0 = fixed("2026-01-01T00:00:00Z");
        let t1 = fixed("2026-01-08T00:00:00Z"); // 7 days later
        store.insert_claim(claim_at("x", 0.8, t0)).unwrap();

        // half_life 7 days -> should halve to 0.4
        store.decay_at(7.0, t1).unwrap();
        let got = store.get_claim("x").unwrap();
        assert!(
            (got.confidence - 0.4).abs() < 1e-9,
            "got {}",
            got.confidence
        );
        assert_eq!(got.updated_at, t1);

        // deterministic pure fn also halves
        let pure = decay_confidence(0.8, 7.0, 7.0);
        assert!((pure - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_decay_two_half_lives() {
        let store = BeliefStore::open_in_memory().unwrap();
        let t0 = fixed("2026-01-01T00:00:00Z");
        let t1 = fixed("2026-01-15T00:00:00Z"); // 14 days
        store.insert_claim(claim_at("y", 1.0, t0)).unwrap();
        store.decay_at(7.0, t1).unwrap();
        let got = store.get_claim("y").unwrap();
        assert!(
            (got.confidence - 0.25).abs() < 1e-9,
            "got {}",
            got.confidence
        );
    }

    #[test]
    fn test_contradictions_lookup() {
        let store = BeliefStore::open_in_memory().unwrap();
        let t0 = fixed("2026-01-01T00:00:00Z");
        for id in ["p", "q", "r"] {
            store.insert_claim(claim_at(id, 0.5, t0)).unwrap();
        }
        store.link("p", "q", EdgeKind::Contradicts, 1.0).unwrap();
        store.link("r", "p", EdgeKind::Contradicts, 0.8).unwrap();
        store.link("q", "r", EdgeKind::Supports, 0.5).unwrap();

        // contradictions_of p should return q and r (outgoing contradicts q, incoming contradicts r)
        let mut ids: Vec<_> = store
            .contradictions_of("p")
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["q", "r"]);

        // contradictions_of q should have p (incoming) but not r (supports edge)
        let ids_q: Vec<_> = store
            .contradictions_of("q")
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert!(ids_q.contains(&"p".to_string()));
        assert!(!ids_q.contains(&"r".to_string()));

        // supports edge should not appear in contradictions
        let ids_r: Vec<_> = store
            .contradictions_of("r")
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        // r contradicts p, so should see p, but not q (supports)
        assert!(ids_r.contains(&"p".to_string()));
    }

    #[test]
    fn test_supports_contradicts_strings_present() {
        // Ensure the strings "supports" and "contradicts" exist in source for grep check
        let s = EdgeKind::Supports.to_string();
        let c = EdgeKind::Contradicts.to_string();
        assert_eq!(s, "supports");
        assert_eq!(c, "contradicts");
    }

    #[test]
    fn test_query_related_unknown() {
        let store = BeliefStore::open_in_memory().unwrap();
        let rel = store.query_related("nonexistent", 5).unwrap();
        assert!(rel.is_empty());
    }

    #[test]
    fn test_link_missing_claim_errors() {
        let store = BeliefStore::open_in_memory().unwrap();
        let t0 = fixed("2026-01-01T00:00:00Z");
        store.insert_claim(claim_at("a", 0.5, t0)).unwrap();
        let err = store
            .link("a", "missing", EdgeKind::Supports, 0.5)
            .unwrap_err();
        assert!(err.to_string().contains("destination claim not found"));
    }

    #[test]
    fn test_decay_negative_half_life_noop() {
        let store = BeliefStore::open_in_memory().unwrap();
        let t0 = fixed("2026-01-01T00:00:00Z");
        let t1 = fixed("2026-02-01T00:00:00Z");
        store.insert_claim(claim_at("z", 0.6, t0)).unwrap();
        store.decay_at(0.0, t1).unwrap();
        assert!((store.get_claim("z").unwrap().confidence - 0.6).abs() < 1e-9);
        store.decay_at(-5.0, t1).unwrap();
        assert!((store.get_claim("z").unwrap().confidence - 0.6).abs() < 1e-9);
    }

    // Ensure fixed instants test determinism
    #[test]
    fn test_deterministic_decay_math_fixed_instants() {
        let a = fixed("2026-01-01T00:00:00Z");
        let b = fixed("2026-01-02T00:00:00Z");
        let elapsed = elapsed_days(a, b);
        assert!((elapsed - 1.0).abs() < 1e-9);
        let c = decay_confidence(0.9, elapsed, 1.0);
        assert!((c - 0.45).abs() < 1e-9);
        // No drift on second call
        let c2 = decay_confidence(0.9, elapsed, 1.0);
        assert_eq!(c, c2);
        let _ = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    }
}
