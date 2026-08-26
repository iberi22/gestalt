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

/// A single fragment/document to be indexed — mirrors the "fragments in" flow
/// from the spec (id, content, project, ts).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fragment {
    /// Unique document id.
    pub id: String,
    /// Logical path or origin.
    pub path: String,
    /// Full text content for BM25.
    pub content: String,
    /// Category filter (kind/project namespace).
    pub kind: String,
    /// Optional project namespace (maps to `project` field in spec).
    pub project: Option<String>,
    /// Optional timestamp (unix millis) for `ts` field.
    pub timestamp: Option<i64>,
}

impl Fragment {
    /// Create a minimal fragment.
    pub fn new(id: impl Into<String>, path: impl Into<String>, content: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            content: content.into(),
            kind: kind.into(),
            project: None,
            timestamp: None,
        }
    }

    /// With project namespace.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// With timestamp.
    pub fn with_timestamp(mut self, ts: i64) -> Self {
        self.timestamp = Some(ts);
        self
    }
}

/// The local Tantivy BM25 search engine.
///
/// Documents are indexed with fields:
/// - `id` (stored, unique)
/// - `path` (stored, path/origin)
/// - `content` (text indexed for BM25)
/// - `kind` (stored + text, category filter)
/// - `project` (stored, namespace — spec field `project`)
/// - `timestamp` (stored + fast, unix millis — spec field `ts`)
///
/// The index directory is configurable and persists across restarts (mmap reopen).
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
    project: Field,
    timestamp: Field,
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

        let schema = build_schema();
        let index_exists = index_dir_path.join("meta.json").exists();

        let (index, fields) = if index_exists {
            info!(
                "Opening existing Tantivy index at {}",
                index_dir_path.display()
            );
            let index = Index::open_in_dir(&index_dir_path)?;
            let schema_ref = index.schema().clone();
            let fields = resolve_fields(&schema_ref)?;
            (index, fields)
        } else {
            info!("Creating new Tantivy index at {}", index_dir_path.display());
            let index = Index::create_in_dir(&index_dir_path, schema.clone())?;
            let fields = resolve_fields(&schema)?;
            (index, fields)
        };

        // Register a default tokenizer with lowercasing
        // Note: stemming via `tantivy`'s default en stemmer requires extra setup;
        // we keep SimpleTokenizer + LowerCaser for deterministic behavior.
        let tokenizer = TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(LowerCaser)
            .build();
        // Register is idempotent; ignore if already registered (reopen case).
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

        let schema_arc = Arc::new(index.schema().clone());
        Ok(Self {
            index,
            writer: Arc::new(Mutex::new(writer)),
            reader,
            schema: schema_arc,
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

    /// Batch-index multiple fragments atomically (single commit).
    /// This is the "indexed batchingly" path from the spec.
    pub async fn index_batch(&self, fragments: &[Fragment]) -> anyhow::Result<()> {
        if fragments.is_empty() {
            return Ok(());
        }
        let mut writer = self.writer.lock().await;
        for frag in fragments {
            let mut doc = tantivy::TantivyDocument::default();
            doc.add_text(self.fields.id, &frag.id);
            doc.add_text(self.fields.path, &frag.path);
            doc.add_text(self.fields.content, &frag.content);
            doc.add_text(self.fields.kind, &frag.kind);
            if let Some(ref project) = frag.project {
                doc.add_text(self.fields.project, project);
            }
            if let Some(ts) = frag.timestamp {
                doc.add_i64(self.fields.timestamp, ts);
            }
            writer.add_document(doc)?;
        }
        writer.commit()?;
        self.reader.reload()?;
        debug!("Batch-indexed {} fragments", fragments.len());
        Ok(())
    }

    /// Convenience: batch-index from raw tuples (id, path, content, kind).
    pub async fn index_tuples_batch(&self, docs: &[(&str, &str, &str, &str)]) -> anyhow::Result<()> {
        let fragments: Vec<Fragment> = docs
            .iter()
            .map(|(id, path, content, kind)| Fragment::new(*id, *path, *content, *kind))
            .collect();
        self.index_batch(&fragments).await
    }

    /// Returns the index directory path (for persistence verification).
    pub fn index_dir(&self) -> &std::path::Path {
        &self.index_dir_path
    }
}

fn build_schema() -> Schema {
    let mut builder = SchemaBuilder::new();
    builder.add_text_field("id", STRING | STORED);
    builder.add_text_field("path", STRING | STORED);
    builder.add_text_field("content", TEXT | STORED);
    builder.add_text_field("kind", STRING | STORED);
    builder.add_text_field("project", STRING | STORED);
    builder.add_i64_field("timestamp", INDEXED | STORED | FAST);
    builder.build()
}

fn resolve_fields(schema: &Schema) -> anyhow::Result<IndexedFields> {
    let id = schema
        .get_field("id")
        .map_err(|e| anyhow::anyhow!("missing field id: {e}"))?;
    let path = schema
        .get_field("path")
        .map_err(|e| anyhow::anyhow!("missing field path: {e}"))?;
    let content = schema
        .get_field("content")
        .map_err(|e| anyhow::anyhow!("missing field content: {e}"))?;
    let kind = schema
        .get_field("kind")
        .map_err(|e| anyhow::anyhow!("missing field kind: {e}"))?;
    // For backwards compat with indexes created before project/timestamp existed,
    // fallback to existing fields gracefully.
    let project = schema.get_field("project").unwrap_or(id);
    let timestamp = schema.get_field("timestamp").unwrap_or(id);
    // If schema actually lacks project/timestamp, we still return handles but
    // the caller will effectively no-op those fields. New indexes will have them.
    // For new indexes, these are correct distinct fields.
    Ok(IndexedFields {
        id,
        path,
        content,
        kind,
        project,
        timestamp,
    })
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

        // Build document honoring full schema (project/timestamp optional)
        let mut doc = tantivy::TantivyDocument::default();
        doc.add_text(self.fields.id, doc_id);
        doc.add_text(self.fields.path, doc_path);
        doc.add_text(self.fields.content, content);
        doc.add_text(self.fields.kind, kind);
        writer.add_document(doc)?;

        // Commit to make it visible to searchers
        writer.commit()?;
        self.reader.reload()?;

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
        // Empty/wildcard query is safe — return empty rather than parse error.
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        if limit == 0 {
            return Ok(Vec::new());
        }

        let searcher = self.reader.searcher();

        // Build query
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.content, self.fields.path]);

        let parsed_query = match query_parser.parse_query(query) {
            Ok(q) => q,
            Err(e) => {
                // Gracefully handle parse errors (e.g. empty or malformed) per
                // workspace rule: never panic on service errors.
                debug!("query parse error for '{}': {}", query, e);
                return Ok(Vec::new());
            }
        };

        // Search with BM25 ranking
        let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit).order_by_score())?;

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
        self.reader.reload()?;

        debug!("Deleted document id={}", doc_id);
        Ok(())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().await;

        writer.delete_all_documents()?;
        writer.commit()?;
        self.reader.reload()?;

        info!("Cleared all documents from Tantivy index");
        Ok(())
    }

    async fn doc_count(&self) -> anyhow::Result<usize> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs() as usize)
    }
}

mod hybrid;

pub use hybrid::{HybridConfig, HybridSearchEngine, SearchMode};

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

    #[tokio::test]
    async fn test_regression_index_search_filter_delete() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("regression_test"), 1).unwrap();

        // 1. Assert doc_count is initially 0
        assert_eq!(engine.doc_count().await.unwrap(), 0);

        // 2. Index ≥ 2 docs
        engine
            .index_document(
                "doc_1",
                "folder/doc1.md",
                "Machine learning and artificial intelligence are revolutionizing search engines",
                "ai_topic",
            )
            .await
            .unwrap();

        engine
            .index_document(
                "doc_2",
                "folder/doc2.md",
                "Traditional search engines rely heavily on keyword indexing and lexical match",
                "search_topic",
            )
            .await
            .unwrap();

        // 3. Assert count is 2
        assert_eq!(engine.doc_count().await.unwrap(), 2);

        // 4. Search and verify matches
        let search_results = engine.search("search engines", 10).await.unwrap();
        assert_eq!(search_results.len(), 2);

        // 5. Filter by kind and assert
        let ai_filtered = engine
            .search_filtered("search engines", Some("ai_topic"), 10)
            .await
            .unwrap();
        assert_eq!(ai_filtered.len(), 1);
        assert_eq!(ai_filtered[0].id, "doc_1");

        // 6. Delete one doc
        engine.delete_document("doc_1").await.unwrap();

        // 7. Assert doc_count drops to 1
        assert_eq!(engine.doc_count().await.unwrap(), 1);

        // 8. Search again and verify only doc_2 remains
        let post_delete_search = engine.search("search engines", 10).await.unwrap();
        assert_eq!(post_delete_search.len(), 1);
        assert_eq!(post_delete_search[0].id, "doc_2");
    }

    // --- New tests for spec acceptance ---

    #[tokio::test]
    async fn test_roundtrip_top1() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("rt"), 1).unwrap();

        engine
            .index_document("doc_a", "a.md", "Rust async runtime tokio is powerful", "code")
            .await
            .unwrap();
        engine
            .index_document("doc_b", "b.md", "Python data science and pandas", "code")
            .await
            .unwrap();
        engine
            .index_document("doc_c", "c.md", "Go concurrency with goroutines", "code")
            .await
            .unwrap();

        // Query unique to doc_a should rank it top-1
        let results = engine.search("tokio runtime", 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "doc_a");
    }

    #[tokio::test]
    async fn test_empty_query_safe() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("empty"), 1).unwrap();

        engine
            .index_document("1", "a.md", "hello world", "test")
            .await
            .unwrap();

        // Empty and whitespace queries must not error and return empty
        assert!(engine.search("", 10).await.unwrap().is_empty());
        assert!(engine.search("   ", 10).await.unwrap().is_empty());
        assert!(engine.search_filtered("", Some("test"), 10).await.unwrap().is_empty());
        assert!(engine.search_filtered("   ", None, 10).await.unwrap().is_empty());
        // Zero limit also safe
        assert!(engine.search("hello", 0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_batch_indexing() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("batch"), 1).unwrap();

        let fragments = vec![
            Fragment::new("1", "a.md", "alpha beta gamma", "code"),
            Fragment::new("2", "b.md", "beta gamma delta", "code"),
            Fragment::new("3", "c.md", "gamma delta epsilon", "plan"),
        ];
        engine.index_batch(&fragments).await.unwrap();
        assert_eq!(engine.doc_count().await.unwrap(), 3);

        let results = engine.search("alpha", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");

        // Batch with project/ts fields
        let frag_with_meta = Fragment::new("4", "d.md", "zeta unique content", "code")
            .with_project("proj-x")
            .with_timestamp(1_700_000_000_000);
        engine.index_batch(&[frag_with_meta]).await.unwrap();
        assert_eq!(engine.doc_count().await.unwrap(), 4);
        let r = engine.search("zeta", 10).await.unwrap();
        assert_eq!(r[0].id, "4");
    }

    #[tokio::test]
    async fn test_persistence_mmap_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("persist");
        {
            let engine = TantivySearchEngine::new(path.clone(), 1).unwrap();
            engine
                .index_document("persist_1", "p.md", "persistent lexical content", "test")
                .await
                .unwrap();
            assert_eq!(engine.doc_count().await.unwrap(), 1);
        }
        // Reopen via new instance on same path (mmap reopen)
        let engine2 = TantivySearchEngine::new(path.clone(), 1).unwrap();
        assert_eq!(engine2.doc_count().await.unwrap(), 1);
        let results = engine2.search("persistent", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "persist_1");
    }

    #[tokio::test]
    async fn test_malformed_query_safe() {
        let dir = tempdir().unwrap();
        let engine = TantivySearchEngine::new(dir.path().join("malformed"), 1).unwrap();
        engine
            .index_document("1", "a.md", "hello world", "test")
            .await
            .unwrap();
        // Tantivy query parser may error on special chars; should return Ok(empty) not Err
        let r = engine.search("AND OR NOT", 10).await.unwrap();
        // Either empty or at least not error
        assert!(r.len() <= 1);
    }
}
