//! Local Search Engine Port
//!
//! Defines the trait for local text search (BM25 / lexical) used by Gestalt
//! when Xavier remote is unavailable. This is the offline search port.

use async_trait::async_trait;

/// A search result from a local or remote search engine.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Unique identifier for the document.
    pub id: String,
    /// Document path or source reference.
    pub path: String,
    /// The stored content (full text or snippet).
    pub content: String,
    /// A highlighted excerpt, if available.
    pub snippet: String,
    /// Relevance score (higher = more relevant).
    pub score: f64,
}

/// Interface for local lexical (BM25) search.
///
/// Implementations provide full-text search over indexed documents,
/// typically using an inverted index (Tantivy, etc.).
#[async_trait]
pub trait LocalSearchEngine: Send + Sync {
    /// Index a document for search.
    ///
    /// # Arguments
    /// * `id` — Unique document identifier.
    /// * `path` — Logical path / source reference for the document.
    /// * `content` — Full text content to index.
    /// * `kind` — Document category (e.g. "memory", "run_result", "plan").
    async fn index_document(
        &self,
        id: &str,
        path: &str,
        content: &str,
        kind: &str,
    ) -> anyhow::Result<()>;

    /// Search the index with a BM25 query.
    ///
    /// # Arguments
    /// * `query` — Free-text search query.
    /// * `limit` — Maximum number of results.
    ///
    /// # Returns
    /// A ranked list of [`SearchResult`] scored by BM25 relevance.
    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>>;

    /// Search with an optional kind filter.
    async fn search_filtered(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>>;

    /// Remove a document from the index by its id.
    async fn delete_document(&self, id: &str) -> anyhow::Result<()>;

    /// Clear the entire index.
    async fn clear(&self) -> anyhow::Result<()>;

    /// Total number of indexed documents.
    async fn doc_count(&self) -> anyhow::Result<usize>;
}
