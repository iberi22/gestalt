//! Hybrid Search Engine — routes queries between local BM25 and Xavier remote.
//!
//! The [`HybridSearchEngine`] wraps both a [`LocalSearchEngine`] (Tantivy BM25)
//! and a remote Xavier client. It prefers the local engine for speed and offline
//! operation, falling back to Xavier when the local index is empty or the user
//! explicitly requests "remote" mode.

use async_trait::async_trait;
use gestalt_core::application::agent::xavier::XavierClient;
use gestalt_core::ports::outbound::search::{LocalSearchEngine, SearchResult};
use std::sync::Arc;
use tracing::{info, warn};

/// Search mode preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Use local BM25 only (offline).
    Local,
    /// Use Xavier remote only.
    Remote,
    /// Try local first; fall back to remote if local is empty.
    Hybrid,
}

/// A search engine that combines local Tantivy BM25 with remote Xavier.
///
/// # Strategy
///
/// - **`SearchMode::Local`** — Queries only the local Tantivy index.
/// - **`SearchMode::Remote`** — Delegates to Xavier HTTP API.
/// - **`SearchMode::Hybrid`** — Queries local first. If results are empty,
///   falls back to Xavier. Results are merged and deduplicated by id.
pub struct HybridSearchEngine {
    local: Arc<dyn LocalSearchEngine>,
    remote: Option<XavierClient>,
    mode: SearchMode,
    local_weight: f64,
    remote_weight: f64,
}

impl HybridSearchEngine {
    /// Create a new hybrid engine.
    ///
    /// # Arguments
    /// * `local` — The local BM25 search engine (Tantivy).
    /// * `remote` — Optional Xavier HTTP client.
    /// * `mode` — Search mode preference.
    /// * `local_weight` — Relative weight for local BM25 results.
    /// * `remote_weight` — Relative weight for remote Xavier vector results.
    pub fn new(
        local: Arc<dyn LocalSearchEngine>,
        remote: Option<XavierClient>,
        mode: SearchMode,
        local_weight: f64,
        remote_weight: f64,
    ) -> Self {
        Self {
            local,
            remote,
            mode,
            local_weight,
            remote_weight,
        }
    }

    /// Set the search mode at runtime.
    pub fn set_mode(&mut self, mode: SearchMode) {
        self.mode = mode;
    }

    /// Current search mode.
    pub fn mode(&self) -> SearchMode {
        self.mode
    }

    /// Whether Xavier is available.
    pub async fn is_xavier_available(&self) -> bool {
        match &self.remote {
            Some(client) => client.is_available().await,
            None => false,
        }
    }

    /// Index a document in both local and (if available) remote stores.
    pub async fn index_document_both(
        &self,
        id: &str,
        path: &str,
        content: &str,
        kind: &str,
    ) -> anyhow::Result<()> {
        // Always index locally
        self.local.index_document(id, path, content, kind).await?;

        // Also try remote if available
        if let Some(ref xavier) = self.remote {
            if xavier.is_available().await {
                match xavier.add(content, path, kind, serde_json::json!({})).await {
                    Ok(_) => info!("Indexed document {} in Xavier", id),
                    Err(e) => warn!("Failed to index in Xavier (non-fatal): {}", e),
                }
            }
        }

        Ok(())
    }

    // --- helpers for Xavier bridging ---

    async fn search_remote(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        match &self.remote {
            Some(client) => {
                // Xavier returns mode="hybrid" results by default
                match client.search(query, limit, "hybrid").await {
                    Ok(resp) => resp
                        .results
                        .into_iter()
                        .map(|r| SearchResult {
                            id: r.id,
                            path: r.path,
                            content: r.content,
                            snippet: r.snippet,
                            score: r.score,
                        })
                        .collect(),
                    Err(e) => {
                        warn!("Xavier remote search failed: {}", e);
                        Vec::new()
                    },
                }
            },
            None => Vec::new(),
        }
    }

    async fn search_remote_filtered(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let results = self.search_remote(query, limit).await;
        if let Some(filter_kind) = kind {
            results
                .into_iter()
                .filter(|r| {
                    // Xavier results have `kind` in metadata — we approximate
                    // by checking the path prefix or id pattern
                    r.path.contains(filter_kind) || r.id.contains(filter_kind)
                })
                .collect()
        } else {
            results
        }
    }

    /// Search both local and remote, normalize their scores, apply weights,
    /// and merge/deduplicate them into a single sorted result set.
    pub async fn search_hybrid(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let local_results = self.local.search_filtered(query, kind, limit).await?;
        let remote_results = self.search_remote_filtered(query, kind, limit).await;

        let normalized_local = normalize_scores(&local_results);
        let normalized_remote = normalize_scores(&remote_results);

        let mut weighted_local = normalized_local;
        for r in &mut weighted_local {
            r.score *= self.local_weight;
        }

        let mut weighted_remote = normalized_remote;
        for r in &mut weighted_remote {
            r.score *= self.remote_weight;
        }

        let merged = merge_and_dedup(weighted_local, weighted_remote);
        let mut results = merged;
        results.truncate(limit);

        Ok(results)
    }
}

#[async_trait]
impl LocalSearchEngine for HybridSearchEngine {
    async fn index_document(
        &self,
        id: &str,
        path: &str,
        content: &str,
        kind: &str,
    ) -> anyhow::Result<()> {
        // Index locally always
        self.local.index_document(id, path, content, kind).await
    }

    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
        self.search_filtered(query, None, limit).await
    }

    async fn search_filtered(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        match self.mode {
            SearchMode::Local => {
                // Local only
                self.local.search_filtered(query, kind, limit).await
            },
            SearchMode::Remote => {
                // Remote only
                Ok(self.search_remote_filtered(query, kind, limit).await)
            },
            SearchMode::Hybrid => self.search_hybrid(query, kind, limit).await,
        }
    }

    async fn delete_document(&self, id: &str) -> anyhow::Result<()> {
        self.local.delete_document(id).await
    }

    async fn clear(&self) -> anyhow::Result<()> {
        self.local.clear().await
    }

    async fn doc_count(&self) -> anyhow::Result<usize> {
        self.local.doc_count().await
    }
}

/// Normalizes a slice of SearchResults' scores into the range [0, 1] using min-max normalization.
/// If all scores are equal, maps them to 1.0.
fn normalize_scores(results: &[SearchResult]) -> Vec<SearchResult> {
    if results.is_empty() {
        return Vec::new();
    }
    let mut min_score = f64::MAX;
    let mut max_score = f64::MIN;
    for r in results {
        if r.score < min_score {
            min_score = r.score;
        }
        if r.score > max_score {
            max_score = r.score;
        }
    }

    let range = max_score - min_score;
    results
        .iter()
        .map(|r| {
            let normalized_score = if range.abs() < 1e-9 {
                1.0
            } else {
                (r.score - min_score) / range
            };
            SearchResult {
                score: normalized_score,
                ..r.clone()
            }
        })
        .collect()
}

/// Merges two vectors of SearchResults, deduplicating by document ID while keeping
/// the result with the highest score. The merged vector is sorted descending by score.
fn merge_and_dedup(local: Vec<SearchResult>, remote: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut merged: std::collections::HashMap<String, SearchResult> =
        std::collections::HashMap::new();

    for r in local {
        merged.insert(r.id.clone(), r);
    }

    for r in remote {
        if let Some(existing) = merged.get_mut(&r.id) {
            if r.score > existing.score {
                *existing = r;
            }
        } else {
            merged.insert(r.id.clone(), r);
        }
    }

    let mut final_results: Vec<SearchResult> = merged.into_values().collect();
    final_results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    final_results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TantivySearchEngine;
    use tempfile::tempdir;

    fn create_local_engine() -> (Arc<dyn LocalSearchEngine>, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hybrid_test");
        std::fs::create_dir_all(&path).unwrap();
        let engine = TantivySearchEngine::new(path, 1).unwrap();
        (Arc::new(engine), dir)
    }

    #[tokio::test]
    async fn test_local_mode() {
        let (local, _dir) = create_local_engine();
        let hybrid = HybridSearchEngine::new(local.clone(), None, SearchMode::Local, 1.0, 1.0);

        local
            .index_document("1", "test.md", "Rust programming", "code")
            .await
            .unwrap();

        let results = hybrid.search("Rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_remote_fallback_empty_local() {
        let (local, _dir) = create_local_engine();
        let hybrid = HybridSearchEngine::new(local.clone(), None, SearchMode::Hybrid, 1.0, 1.0);

        // No documents in local, but no remote either — should return empty
        let results = hybrid.search("anything", 10).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_local_has_results_no_fallback() {
        let (local, _dir) = create_local_engine();
        let hybrid = HybridSearchEngine::new(local.clone(), None, SearchMode::Hybrid, 1.0, 1.0);

        local
            .index_document("1", "doc.md", "BM25 search engine", "code")
            .await
            .unwrap();

        // Should return local results without trying remote
        let results = hybrid.search("BM25", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");
    }

    #[test]
    fn test_score_normalization() {
        // Empty
        let empty_results = normalize_scores(&[]);
        assert!(empty_results.is_empty());

        // Single result
        let single = vec![SearchResult {
            id: "1".to_string(),
            path: "a.md".to_string(),
            content: "hello".to_string(),
            snippet: "hello".to_string(),
            score: 42.0,
        }];
        let normalized_single = normalize_scores(&single);
        assert_eq!(normalized_single.len(), 1);
        assert_eq!(normalized_single[0].score, 1.0);

        // Identical scores
        let identical = vec![
            SearchResult {
                id: "1".to_string(),
                path: "a.md".to_string(),
                content: "hello".to_string(),
                snippet: "hello".to_string(),
                score: 5.0,
            },
            SearchResult {
                id: "2".to_string(),
                path: "b.md".to_string(),
                content: "world".to_string(),
                snippet: "world".to_string(),
                score: 5.0,
            },
        ];
        let normalized_identical = normalize_scores(&identical);
        assert_eq!(normalized_identical.len(), 2);
        assert_eq!(normalized_identical[0].score, 1.0);
        assert_eq!(normalized_identical[1].score, 1.0);

        // Different scores
        let different = vec![
            SearchResult {
                id: "1".to_string(),
                path: "a.md".to_string(),
                content: "hello".to_string(),
                snippet: "hello".to_string(),
                score: 2.0,
            },
            SearchResult {
                id: "2".to_string(),
                path: "b.md".to_string(),
                content: "world".to_string(),
                snippet: "world".to_string(),
                score: 6.0,
            },
            SearchResult {
                id: "3".to_string(),
                path: "c.md".to_string(),
                content: "rust".to_string(),
                snippet: "rust".to_string(),
                score: 10.0,
            },
        ];
        let normalized_different = normalize_scores(&different);
        assert_eq!(normalized_different.len(), 3);
        // Score 2.0 -> min -> 0.0
        // Score 10.0 -> max -> 1.0
        // Score 6.0 -> (6 - 2) / (10 - 2) = 4 / 8 = 0.5
        assert!((normalized_different[0].score - 0.0).abs() < 1e-9);
        assert!((normalized_different[1].score - 0.5).abs() < 1e-9);
        assert!((normalized_different[2].score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_merge_and_deduplication() {
        let local = vec![
            SearchResult {
                id: "1".to_string(),
                path: "a.md".to_string(),
                content: "hello".to_string(),
                snippet: "hello".to_string(),
                score: 0.8,
            },
            SearchResult {
                id: "2".to_string(),
                path: "b.md".to_string(),
                content: "world".to_string(),
                snippet: "world".to_string(),
                score: 0.3,
            },
        ];

        let remote = vec![
            SearchResult {
                id: "2".to_string(),
                path: "b.md".to_string(),
                content: "world".to_string(),
                snippet: "world".to_string(),
                score: 0.9, // Higher score than local
            },
            SearchResult {
                id: "3".to_string(),
                path: "c.md".to_string(),
                content: "rust".to_string(),
                snippet: "rust".to_string(),
                score: 0.5,
            },
        ];

        let merged = merge_and_dedup(local, remote);
        // Expecting 3 results: id 2 (score 0.9), id 1 (score 0.8), id 3 (score 0.5)
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].id, "2");
        assert_eq!(merged[0].score, 0.9);
        assert_eq!(merged[1].id, "1");
        assert_eq!(merged[1].score, 0.8);
        assert_eq!(merged[2].id, "3");
        assert_eq!(merged[2].score, 0.5);
    }

    struct MockLocalSearchEngine {
        results: Vec<SearchResult>,
    }

    #[async_trait]
    impl LocalSearchEngine for MockLocalSearchEngine {
        async fn index_document(
            &self,
            _id: &str,
            _path: &str,
            _content: &str,
            _kind: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn search(&self, _query: &str, _limit: usize) -> anyhow::Result<Vec<SearchResult>> {
            Ok(self.results.clone())
        }
        async fn search_filtered(
            &self,
            _query: &str,
            _kind: Option<&str>,
            _limit: usize,
        ) -> anyhow::Result<Vec<SearchResult>> {
            Ok(self.results.clone())
        }
        async fn delete_document(&self, _id: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn clear(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn doc_count(&self) -> anyhow::Result<usize> {
            Ok(self.results.len())
        }
    }

    #[tokio::test]
    async fn test_weighted_hybrid_search() {
        let mock_local = Arc::new(MockLocalSearchEngine {
            results: vec![
                SearchResult {
                    id: "1".to_string(),
                    path: "a.md".to_string(),
                    content: "hello".to_string(),
                    snippet: "hello".to_string(),
                    score: 2.0,
                },
                SearchResult {
                    id: "2".to_string(),
                    path: "b.md".to_string(),
                    content: "world".to_string(),
                    snippet: "world".to_string(),
                    score: 10.0,
                },
            ],
        });

        // Set weights: local_weight = 0.5, remote_weight = 0.8
        let hybrid = HybridSearchEngine::new(mock_local, None, SearchMode::Hybrid, 0.5, 0.8);

        // Let's search
        // We expect local search to return 2 results with different scores.
        // Under SearchMode::Hybrid, search_hybrid is called.
        // Since remote is None, remote results are empty.
        // Local results are normalized (lowest -> 0.0, highest -> 1.0).
        // Then multiplied by local_weight (0.5), yielding 0.0 and 0.5.
        let results = hybrid.search("Rust", 10).await.unwrap();

        // Let's assert that we have exactly 2 results
        assert_eq!(results.len(), 2);

        // Since results are sorted descending, the highest score is first.
        // It should have score 0.5 (1.0 * local_weight).
        assert_eq!(results[0].id, "2");
        assert!((results[0].score - 0.5).abs() < 0.001);

        // The lowest score is last, it should have score 0.0 (0.0 * local_weight).
        assert_eq!(results[1].id, "1");
        assert!((results[1].score - 0.0).abs() < 0.001);
    }
}
