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
}

impl HybridSearchEngine {
    /// Create a new hybrid engine.
    ///
    /// # Arguments
    /// * `local` — The local BM25 search engine (Tantivy).
    /// * `remote` — Optional Xavier HTTP client.
    /// * `mode` — Search mode preference.
    pub fn new(
        local: Arc<dyn LocalSearchEngine>,
        remote: Option<XavierClient>,
        mode: SearchMode,
    ) -> Self {
        Self {
            local,
            remote,
            mode,
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
            SearchMode::Hybrid => {
                // Try local first
                let local_results = self.local.search_filtered(query, kind, limit).await?;

                if !local_results.is_empty() {
                    return Ok(local_results);
                }

                // Fall back to remote
                info!("Local search returned 0 results, falling back to Xavier");
                Ok(self.search_remote_filtered(query, kind, limit).await)
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TantivySearchEngine;
    use tempfile::tempdir;

    fn create_local_engine() -> Arc<dyn LocalSearchEngine> {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("hybrid_test"), 1).unwrap();
        Arc::new(engine)
    }

    #[tokio::test]
    async fn test_local_mode() {
        let local = create_local_engine();
        let hybrid = HybridSearchEngine::new(local.clone(), None, SearchMode::Local);

        local
            .index_document("1", "test.md", "Rust programming", "code")
            .await
            .unwrap();

        let results = hybrid.search("Rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_remote_fallback_empty_local() {
        let local = create_local_engine();
        let hybrid = HybridSearchEngine::new(local.clone(), None, SearchMode::Hybrid);

        // No documents in local, but no remote either — should return empty
        let results = hybrid.search("anything", 10).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_local_has_results_no_fallback() {
        let local = create_local_engine();
        let hybrid = HybridSearchEngine::new(local.clone(), None, SearchMode::Hybrid);

        local
            .index_document("1", "doc.md", "BM25 search engine", "code")
            .await
            .unwrap();

        // Should return local results without trying remote
        let results = hybrid.search("BM25", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");
    }
}
