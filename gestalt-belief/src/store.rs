//! Persistence + in-memory petgraph store for the belief graph.
//! Mirrors `gestalt-state` SQLite style: WAL mode, foreign_keys, same connection pattern.

use crate::model::{decay_confidence, elapsed_days, BeliefEdge, Claim, EdgeKind};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// SQLite-backed belief store with an in-memory petgraph overlay.
///
/// The graph is kept in sync with SQLite on every write. On open the graph
/// is rehydrated from the persisted tables so restart is lossless.
pub struct BeliefStore {
    conn: Arc<Mutex<Connection>>,
    graph: Arc<Mutex<DiGraph<Claim, BeliefEdge>>>,
    /// Map claim id -> NodeIndex for fast lookup.
    index: Arc<Mutex<HashMap<String, NodeIndex>>>,
}

impl BeliefStore {
    /// Open (or create) a SQLite DB at `path` and hydrate the in-memory graph.
    ///
    /// Enables WAL + foreign_keys mirroring `gestalt-state::StateDb::open`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent dir for {}", path.display())
                })?;
            }
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite DB at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMAs")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            graph: Arc::new(Mutex::new(DiGraph::new())),
            index: Arc::new(Mutex::new(HashMap::new())),
        };
        store.migrate()?;
        store.hydrate()?;
        Ok(store)
    }

    /// Open an in-memory store (for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory DB")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMAs")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            graph: Arc::new(Mutex::new(DiGraph::new())),
            index: Arc::new(Mutex::new(HashMap::new())),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock poisoned: {e}"))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS claims (
                id TEXT PRIMARY KEY,
                statement TEXT NOT NULL,
                confidence REAL NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS edges (
                src TEXT NOT NULL,
                dst TEXT NOT NULL,
                kind TEXT NOT NULL,
                weight REAL NOT NULL,
                PRIMARY KEY (src, dst, kind),
                FOREIGN KEY (src) REFERENCES claims(id) ON DELETE CASCADE,
                FOREIGN KEY (dst) REFERENCES claims(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_edges_src ON edges(src);
            CREATE INDEX IF NOT EXISTS idx_edges_dst ON edges(dst);
            CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
            ",
        )
        .context("Failed to run BeliefStore migrations")?;
        Ok(())
    }

    fn hydrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock poisoned: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT id, statement, confidence, updated_at FROM claims")
            .context("prepare claims hydrate")?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let statement: String = row.get(1)?;
                let confidence: f64 = row.get(2)?;
                let updated_at_str: String = row.get(3)?;
                Ok((id, statement, confidence, updated_at_str))
            })
            .context("query claims hydrate")?;

        let mut graph = self
            .graph
            .lock()
            .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
        let mut index = self
            .index
            .lock()
            .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
        graph.clear();
        index.clear();

        for row in rows {
            let (id, statement, confidence, updated_at_str) = row.context("read claim row")?;
            let updated_at = updated_at_str
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            let claim = Claim {
                id: id.clone(),
                statement,
                confidence,
                updated_at,
            };
            let ni = graph.add_node(claim);
            index.insert(id, ni);
        }
        drop(graph);
        drop(index);

        // Hydrate edges
        let mut stmt2 = conn
            .prepare("SELECT src, dst, kind, weight FROM edges")
            .context("prepare edges hydrate")?;
        let erows = stmt2
            .query_map([], |row| {
                let src: String = row.get(0)?;
                let dst: String = row.get(1)?;
                let kind: String = row.get(2)?;
                let weight: f64 = row.get(3)?;
                Ok((src, dst, kind, weight))
            })
            .context("query edges hydrate")?;

        let mut graph = self
            .graph
            .lock()
            .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
        let index = self
            .index
            .lock()
            .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
        for row in erows {
            let (src, dst, kind_str, weight) = row.context("read edge row")?;
            let kind: EdgeKind = kind_str
                .parse()
                .map_err(|e: String| rusqlite::Error::InvalidParameterName(e))?;
            if let (Some(&s_idx), Some(&d_idx)) = (index.get(&src), index.get(&dst)) {
                graph.add_edge(s_idx, d_idx, BeliefEdge { kind, weight });
            }
        }
        Ok(())
    }

    /// Insert or replace a claim.
    pub fn insert_claim(&self, claim: Claim) -> Result<()> {
        // Persist to SQLite
        {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("DB lock poisoned: {e}"))?;
            conn.execute(
                "INSERT INTO claims (id, statement, confidence, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET statement=excluded.statement, confidence=excluded.confidence, updated_at=excluded.updated_at",
                rusqlite::params![claim.id, claim.statement, claim.confidence, claim.updated_at.to_rfc3339()],
            )
            .context("Failed to insert claim")?;
        }
        // Update graph
        {
            let mut graph = self
                .graph
                .lock()
                .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
            let mut index = self
                .index
                .lock()
                .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
            if let Some(&ni) = index.get(&claim.id) {
                graph[ni] = claim.clone();
            } else {
                let ni = graph.add_node(claim.clone());
                index.insert(claim.id, ni);
            }
        }
        Ok(())
    }

    /// Fetch a claim by id (from in-memory graph).
    pub fn get_claim(&self, id: &str) -> Option<Claim> {
        let index = self.index.lock().ok()?;
        let ni = *index.get(id)?;
        let graph = self.graph.lock().ok()?;
        Some(graph[ni].clone())
    }

    /// Number of claims in the graph.
    pub fn claim_count(&self) -> usize {
        self.graph.lock().map(|g| g.node_count()).unwrap_or(0)
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.lock().map(|g| g.edge_count()).unwrap_or(0)
    }

    /// Create a typed directed edge from `src` to `dst`.
    ///
    /// Both endpoints must exist; otherwise an error is returned.
    #[allow(clippy::too_many_arguments)]
    pub fn link(&self, src: &str, dst: &str, kind: EdgeKind, weight: f64) -> Result<()> {
        // Validate endpoints exist
        {
            let index = self
                .index
                .lock()
                .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
            if !index.contains_key(src) {
                anyhow::bail!("source claim not found: {src}");
            }
            if !index.contains_key(dst) {
                anyhow::bail!("destination claim not found: {dst}");
            }
        }
        // Persist
        {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("DB lock poisoned: {e}"))?;
            conn.execute(
                "INSERT INTO edges (src, dst, kind, weight) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(src, dst, kind) DO UPDATE SET weight=excluded.weight",
                rusqlite::params![src, dst, kind.to_string(), weight],
            )
            .context("Failed to insert edge")?;
        }
        // Update graph: remove existing edge of same kind if present, then add.
        {
            let mut graph = self
                .graph
                .lock()
                .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
            let index = self
                .index
                .lock()
                .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
            let s_idx = *index.get(src).unwrap();
            let d_idx = *index.get(dst).unwrap();
            // Remove any existing edge s->d with same kind
            let existing = graph
                .edges_connecting(s_idx, d_idx)
                .find(|e| e.weight().kind == kind)
                .map(|e| e.id());
            if let Some(eid) = existing {
                graph.remove_edge(eid);
            }
            graph.add_edge(s_idx, d_idx, BeliefEdge { kind, weight });
        }
        Ok(())
    }

    /// Query claims related to `id` up to `depth` hops (BFS, outgoing edges).
    ///
    /// Returns the starting claim plus all reachable claims within depth.
    /// Does not include duplicates. Returns empty if id not found.
    pub fn query_related(&self, id: &str, depth: usize) -> Result<Vec<Claim>> {
        let index = self
            .index
            .lock()
            .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
        let ni = match index.get(id) {
            Some(v) => *v,
            None => return Ok(Vec::new()),
        };
        drop(index);
        let graph = self
            .graph
            .lock()
            .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        let mut result: Vec<Claim> = Vec::new();
        let mut frontier: Vec<NodeIndex> = vec![ni];
        visited.insert(ni);
        result.push(graph[ni].clone());
        let mut current_depth = 0;
        while current_depth < depth && !frontier.is_empty() {
            let mut next_frontier = Vec::new();
            for &node in &frontier {
                for neighbor in graph.neighbors(node) {
                    if visited.insert(neighbor) {
                        result.push(graph[neighbor].clone());
                        next_frontier.push(neighbor);
                    }
                }
            }
            frontier = next_frontier;
            current_depth += 1;
        }
        Ok(result)
    }

    /// Return claims that contradict the given claim (incoming or outgoing `contradicts` edges).
    ///
    /// Includes both directions: A contradicts B and B contradicts A are both considered contradictions.
    pub fn contradictions_of(&self, id: &str) -> Result<Vec<Claim>> {
        let index = self
            .index
            .lock()
            .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
        let ni = match index.get(id) {
            Some(v) => *v,
            None => return Ok(Vec::new()),
        };
        let graph = self
            .graph
            .lock()
            .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
        let mut out = Vec::new();
        let mut seen: HashSet<NodeIndex> = HashSet::new();
        // Outgoing contradicts
        for edge in graph.edges(ni) {
            if edge.weight().kind == EdgeKind::Contradicts {
                let target = edge.target();
                if seen.insert(target) {
                    out.push(graph[target].clone());
                }
            }
        }
        // Incoming contradicts
        for edge in graph.edges_directed(ni, petgraph::Direction::Incoming) {
            if edge.weight().kind == EdgeKind::Contradicts {
                let source = edge.source();
                if seen.insert(source) {
                    out.push(graph[source].clone());
                }
            }
        }
        Ok(out)
    }

    /// Apply half-life decay to all claims relative to `now`.
    ///
    /// For each claim, `new_conf = decay_confidence(old_conf, elapsed_days, half_life_days)` where
    /// `elapsed_days = now - claim.updated_at`. After decay, `updated_at` is set to `now` so
    /// repeated decay without new evidence compounds correctly only on next interval.
    pub fn decay_at(&self, half_life_days: f64, now: DateTime<Utc>) -> Result<()> {
        if half_life_days <= 0.0 {
            return Ok(());
        }
        // Collect updates
        let updates: Vec<(String, f64)> = {
            let graph = self
                .graph
                .lock()
                .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
            graph
                .node_weights()
                .map(|c| {
                    let elapsed = elapsed_days(c.updated_at, now);
                    let new_conf = decay_confidence(c.confidence, elapsed, half_life_days);
                    (c.id.clone(), new_conf)
                })
                .collect()
        };
        // Persist updates
        {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("DB lock poisoned: {e}"))?;
            for (id, new_conf) in &updates {
                conn.execute(
                    "UPDATE claims SET confidence = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![new_conf, now.to_rfc3339(), id],
                )
                .context("Failed to decay claim")?;
            }
        }
        // Update graph
        {
            let mut graph = self
                .graph
                .lock()
                .map_err(|e| anyhow::anyhow!("graph lock poisoned: {e}"))?;
            let index = self
                .index
                .lock()
                .map_err(|e| anyhow::anyhow!("index lock poisoned: {e}"))?;
            for (id, new_conf) in updates {
                if let Some(&ni) = index.get(id.as_str()) {
                    graph[ni].confidence = new_conf;
                    graph[ni].updated_at = now;
                }
            }
        }
        Ok(())
    }

    /// Convenience: decay using current wall-clock time.
    pub fn decay(&self, half_life_days: f64) -> Result<()> {
        self.decay_at(half_life_days, Utc::now())
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;

    fn fixed(s: &str) -> DateTime<Utc> {
        s.parse::<DateTime<Utc>>().unwrap()
    }

    fn claim_at(id: &str, conf: f64, ts: DateTime<Utc>) -> Claim {
        Claim {
            id: id.to_string(),
            statement: format!("statement {id}"),
            confidence: conf,
            updated_at: ts,
        }
    }

    #[test]
    fn test_insert_get_roundtrip() {
        let store = BeliefStore::open_in_memory().unwrap();
        let c = claim_at("c1", 0.9, fixed("2026-01-01T00:00:00Z"));
        store.insert_claim(c.clone()).unwrap();
        let got = store.get_claim("c1").unwrap();
        assert_eq!(got.id, "c1");
        assert!((got.confidence - 0.9).abs() < 1e-9);
    }

    #[test]
    fn test_persistence_rehydrate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("belief.db");
        {
            let store = BeliefStore::open(&path).unwrap();
            store
                .insert_claim(claim_at("a", 0.8, fixed("2026-01-01T00:00:00Z")))
                .unwrap();
            store
                .insert_claim(claim_at("b", 0.5, fixed("2026-01-01T00:00:00Z")))
                .unwrap();
            store.link("a", "b", EdgeKind::Supports, 0.7).unwrap();
            assert_eq!(store.claim_count(), 2);
            assert_eq!(store.edge_count(), 1);
        }
        // Reopen
        let store2 = BeliefStore::open(&path).unwrap();
        assert_eq!(store2.claim_count(), 2);
        assert_eq!(store2.edge_count(), 1);
        let rel = store2.query_related("a", 1).unwrap();
        assert_eq!(rel.len(), 2);
    }
}
