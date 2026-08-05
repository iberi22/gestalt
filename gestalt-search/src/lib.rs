//! gestalt-search — Local BM25 Full-Text Search via Tantivy
//!
//! Provides a local Tantivy-based BM25 search engine that implements
//! [`LocalSearchEngine`] for offline operation when Xavier is unreachable.

use async_trait::async_trait;
use gestalt_core::ports::outbound::search::{LocalSearchEngine, SearchResult};
use std::path::PathBuf;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::tokenizer::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Errors from the local Tantivy search engine.
#[derive(Debug, Error)]
pub enum TantivySearchError {
    /// Tantivy internal error.
    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    /// I/O error (index directory, etc.).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Query parse error.
    #[error("Query parse error: {0}")]
    QueryParse(#[from] tantivy::query::QueryParserError),

    /// Serde error.
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// The local Tantivy BM25 search engine.
///
/// Documents are indexed with fields:
/// - `id` (stored, unique)
/// - `path` (stored, path/origin)
/// - `content` (text indexed for BM25)
/// - `kind` (stored + text, category filter)
///
/// The index directory is configurable and persists across restarts.
pub struct TantivySearchEngine {
    index: Index,
    writer: Arc<Mutex<IndexWriter>>,
    reader: IndexReader,
    schema: Arc<Schema>,
    /// Field handles for fast access.
    fields: IndexedFields,
    /// The directory path of the index on disk (stored for rebuild).
    index_dir_path: PathBuf,
}

struct IndexedFields {
    id: Field,
    path: Field,
    content: Field,
    kind: Field,
}

impl TantivySearchEngine {
    /// Create or open a Tantivy index at the given directory path.
    ///
    /// # Arguments
    /// * `index_dir` — Directory where the Tantivy index lives or will be created.
    /// * `num_threads` — Number of indexing threads (0 = auto).
    pub fn new(index_dir: impl Into<PathBuf>, _num_threads: usize) -> anyhow::Result<Self> {
        let index_dir_path: PathBuf = index_dir.into();

        // Ensure the directory exists
        std::fs::create_dir_all(&index_dir_path)?;

        let mut schema_builder = SchemaBuilder::new();

        // Field definitions
        let id = schema_builder.add_text_field("id", STRING | STORED);
        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let content = schema_builder.add_text_field("content", TEXT | STORED);
        let kind = schema_builder.add_text_field("kind", STRING | STORED);

        let schema = schema_builder.build();
        let fields = IndexedFields {
            id,
            path: path_field,
            content,
            kind,
        };

        let index = if index_dir_path.join("meta.json").exists() {
            info!("Opening existing Tantivy index at {}", index_dir_path.display());
            Index::open_in_dir(&index_dir_path)?
        } else {
            info!("Creating new Tantivy index at {}", index_dir_path.display());
            Index::create_in_dir(&index_dir_path, schema.clone())?
        };

        // Register a default tokenizer with lowercasing + stemming
        let tokenizer = TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(LowerCaser)
            .build();
        index.tokenizers().register("default", tokenizer);

        // Create writer with configurable memory (50 MB by default)
        let writer = index
            .writer(50_000_000)
            .map_err(TantivySearchError::Tantivy)?;

        // Create reader with auto-reload
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            writer: Arc::new(Mutex::new(writer)),
            reader,
            schema: Arc::new(schema),
            fields,
            index_dir_path,
        })
    }

    /// Rebuild the index from scratch. All existing documents are removed.
    pub fn rebuild(self) -> anyhow::Result<Self> {
        let dir = self.index_dir_path.clone();

        // Close the current index by dropping everything
        drop(self);

        // Clear directory and recreate
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Self::new(dir, 1)
    }
}

#[async_trait]
impl LocalSearchEngine for TantivySearchEngine {
    async fn index_document(
        &self,
        doc_id: &str,
        doc_path: &str,
        content: &str,
        kind: &str,
    ) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().await;

        // Add document
        writer.add_document(doc!(
            self.fields.id => doc_id,
            self.fields.path => doc_path,
            self.fields.content => content,
            self.fields.kind => kind,
        ))?;

        // Commit to make it visible to searchers
        writer.commit()?;

        debug!(
            "Indexed document id={} kind={} path={}",
            doc_id, kind, doc_path
        );
        Ok(())
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
        let searcher = self.reader.searcher();

        // Build query
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.content, self.fields.path]);

        let query = query_parser.parse_query(query)?;

        // Search with BM25 ranking
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let doc = searcher.doc::<TantivyDocument>(doc_address)?;

            let id = doc
                .get_first(self.fields.id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let path = doc
                .get_first(self.fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = doc
                .get_first(self.fields.content)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let kind_val = doc
                .get_first(self.fields.kind)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Apply kind filter if specified
            if let Some(filter_kind) = kind {
                if kind_val != filter_kind {
                    continue;
                }
            }

            // Generate a snippet (first 200 chars)
            let snippet = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            };

            results.push(SearchResult {
                id,
                path,
                content,
                snippet,
                score: score as f64,
            });
        }

        Ok(results)
    }

    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().await;

        // Delete by id field term
        let term = tantivy::Term::from_field_text(self.fields.id, doc_id);
        writer.delete_term(term);
        writer.commit()?;

        debug!("Deleted document id={}", doc_id);
        Ok(())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().await;

        // Delete all documents by deleting a non-existent term range
        // (Tantivy doesn't have "delete all", so we recreate)
        // Actually, we can use a wildcard by matching all documents
        writer.delete_all_documents()?;
        writer.commit()?;

        info!("Cleared all documents from Tantivy index");
        Ok(())
    }

    async fn doc_count(&self) -> anyhow::Result<usize> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs() as usize)
    }
}

mod hybrid;

pub use hybrid::{HybridSearchEngine, SearchMode};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_index_and_search() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("search"), 1).unwrap();

        // Index a document
        engine
            .index_document(
                "1",
                "doc1.md",
                "Rust is a systems programming language",
                "code",
            )
            .await
            .unwrap();

        // Search for it
        let results = engine.search("Rust programming", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");
        assert!(results[0].score > 0.0);
    }

    #[tokio::test]
    async fn test_kind_filter() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("search"), 1).unwrap();

        engine
            .index_document("1", "doc1.md", "Python is a dynamic language", "code")
            .await
            .unwrap();
        engine
            .index_document("2", "plan1.md", "Python project roadmap", "plan")
            .await
            .unwrap();

        // Unfiltered should match both
        let all = engine.search("Python", 10).await.unwrap();
        assert_eq!(all.len(), 2);

        // Filtered should only match code
        let code_results = engine
            .search_filtered("Python", Some("code"), 10)
            .await
            .unwrap();
        assert_eq!(code_results.len(), 1);
        assert_eq!(code_results[0].id, "1");
    }

    #[tokio::test]
    async fn test_delete_document() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("search"), 1).unwrap();

        engine
            .index_document("1", "doc1.md", "Rust is great", "code")
            .await
            .unwrap();
        engine
            .index_document("2", "doc2.md", "Rust is fast", "code")
            .await
            .unwrap();

        assert_eq!(engine.search("Rust", 10).await.unwrap().len(), 2);

        engine.delete_document("1").await.unwrap();
        let results = engine.search("Rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "2");
    }

    #[tokio::test]
    async fn test_doc_count() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("search"), 1).unwrap();

        assert_eq!(engine.doc_count().await.unwrap(), 0);

        // Need to wait for a commit to see the count
        engine
            .index_document("1", "a.md", "Hello", "test")
            .await
            .unwrap();
        assert!(engine.doc_count().await.unwrap() >= 1);
    }
}
