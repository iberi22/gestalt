//! Local BM25 (Lexical) Search Engine implementation.
//!
//! Provides a pure-Rust, in-memory BM25 index that supports document indexing,
//! tokenization, inverse document frequency (IDF) calculation, document length tracking,
//! and relevance scoring. It implements the [`LocalSearchEngine`] trait.

use crate::ports::outbound::search::{LocalSearchEngine, SearchResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

/// Standard tokenization function for the BM25 search engine.
///
/// Converts the text to lowercase and splits it on non-alphanumeric characters,
/// filtering out empty tokens.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// A document indexed in BM25.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bm25Document {
    /// Unique document identifier.
    pub id: String,
    /// Logical path or source reference.
    pub path: String,
    /// Full-text content.
    pub content: String,
    /// Category / kind of document.
    pub kind: String,
}

/// In-memory BM25 index structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bm25Index {
    /// Map of document ID to full document metadata.
    pub documents: HashMap<String, Bm25Document>,
    /// Tokenized document contents, mapping doc_id -> list of tokens.
    pub doc_tokens: HashMap<String, Vec<String>>,
    /// Document term frequencies, mapping doc_id -> (term -> frequency).
    pub term_freqs: HashMap<String, HashMap<String, usize>>,
    /// Document frequency of terms (how many documents contain each term).
    pub doc_freqs: HashMap<String, usize>,
    /// Average document length in tokens.
    pub avg_dl: f64,
    /// Total token count across all indexed documents.
    pub total_tokens: usize,
    /// Parameter k1 controls term frequency saturation (typically 1.2 to 2.0).
    pub k1: f64,
    /// Parameter b controls document length normalization (typically 0.75).
    pub b: f64,
}

impl Default for Bm25Index {
    fn default() -> Self {
        Self {
            documents: HashMap::new(),
            doc_tokens: HashMap::new(),
            term_freqs: HashMap::new(),
            doc_freqs: HashMap::new(),
            avg_dl: 0.0,
            total_tokens: 0,
            k1: 1.2,
            b: 0.75,
        }
    }
}

impl Bm25Index {
    /// Create a new empty BM25 index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Indexes a document inside the in-memory BM25 state.
    pub fn index_document(&mut self, id: &str, path: &str, content: &str, kind: &str) {
        // If the document already exists, remove it first to allow updates
        if self.documents.contains_key(id) {
            self.delete_document(id);
        }

        let doc = Bm25Document {
            id: id.to_string(),
            path: path.to_string(),
            content: content.to_string(),
            kind: kind.to_string(),
        };

        let tokens = tokenize(content);
        let mut freqs = HashMap::new();
        let mut unique_terms = HashSet::new();

        for token in &tokens {
            *freqs.entry(token.clone()).or_insert(0) += 1;
            unique_terms.insert(token.clone());
        }

        // Update document frequencies for terms
        for term in unique_terms {
            *self.doc_freqs.entry(term).or_insert(0) += 1;
        }

        self.total_tokens += tokens.len();
        self.term_freqs.insert(id.to_string(), freqs);
        self.doc_tokens.insert(id.to_string(), tokens);
        self.documents.insert(id.to_string(), doc);

        // Recalculate average document length
        let n = self.documents.len();
        if n > 0 {
            self.avg_dl = self.total_tokens as f64 / n as f64;
        } else {
            self.avg_dl = 0.0;
        }
    }

    /// Removes a document from the BM25 index.
    pub fn delete_document(&mut self, id: &str) {
        if self.documents.remove(id).is_some() {
            if let Some(tokens) = self.doc_tokens.remove(id) {
                self.total_tokens -= tokens.len();
            }
            if let Some(freqs) = self.term_freqs.remove(id) {
                for term in freqs.keys() {
                    if let Some(count) = self.doc_freqs.get_mut(term) {
                        if *count > 1 {
                            *count -= 1;
                        } else {
                            self.doc_freqs.remove(term);
                        }
                    }
                }
            }

            // Recalculate average document length
            let n = self.documents.len();
            if n > 0 {
                self.avg_dl = self.total_tokens as f64 / n as f64;
            } else {
                self.avg_dl = 0.0;
            }
        }
    }

    /// Clears the index.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Scores all documents in the index matching the parsed query terms,
    /// returning a ranked list of [`SearchResult`].
    pub fn search(
        &self,
        query_terms: &[String],
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<SearchResult> {
        if self.documents.is_empty() || query_terms.is_empty() {
            return Vec::new();
        }

        let n = self.documents.len() as f64;
        let mut scored_results = Vec::new();

        // Calculate IDF for each query term
        let mut idf_cache = HashMap::new();
        for term in query_terms {
            let df = *self.doc_freqs.get(term).unwrap_or(&0) as f64;
            // Standard BM25 IDF with a floor of 0.0 to prevent negative scoring for very frequent terms
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            let idf = if idf < 0.0 { 0.0 } else { idf };
            idf_cache.insert(term.clone(), idf);
        }

        for (doc_id, doc) in &self.documents {
            // Apply category filter if specified
            if let Some(filter) = kind_filter {
                if doc.kind != filter {
                    continue;
                }
            }

            let doc_len = self.doc_tokens.get(doc_id).map(|t| t.len()).unwrap_or(0) as f64;
            let doc_freqs = match self.term_freqs.get(doc_id) {
                Some(freqs) => freqs,
                None => continue,
            };

            let mut score = 0.0;
            let mut matched_any = false;

            for term in query_terms {
                if let Some(&tf) = doc_freqs.get(term) {
                    matched_any = true;
                    let tf = tf as f64;
                    let idf = *idf_cache.get(term).unwrap_or(&0.0);

                    // BM25 scoring formula:
                    // tf * (k1 + 1) / (tf + k1 * (1 - b + b * (doc_len / avg_dl)))
                    let denom =
                        tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_dl.max(1.0)));
                    let term_score = idf * (tf * (self.k1 + 1.0)) / denom;
                    score += term_score;
                }
            }

            if matched_any {
                // Generate simple snippet (first 200 chars)
                let snippet = if doc.content.len() > 200 {
                    format!("{}...", &doc.content[..200])
                } else {
                    doc.content.clone()
                };

                scored_results.push(SearchResult {
                    id: doc.id.clone(),
                    path: doc.path.clone(),
                    content: doc.content.clone(),
                    snippet,
                    score,
                });
            }
        }

        // Sort descending by score
        scored_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored_results.truncate(limit);
        scored_results
    }
}

/// Thread-safe wrapper for `Bm25Index` that implements `LocalSearchEngine`.
pub struct LocalBm25SearchEngine {
    index: RwLock<Bm25Index>,
}

impl Default for LocalBm25SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalBm25SearchEngine {
    /// Create a new thread-safe BM25 search engine wrapper.
    pub fn new() -> Self {
        Self {
            index: RwLock::new(Bm25Index::new()),
        }
    }

    /// Read-only access to the underlying index.
    pub async fn get_index(&self) -> tokio::sync::RwLockReadGuard<'_, Bm25Index> {
        self.index.read().await
    }
}

#[async_trait]
impl LocalSearchEngine for LocalBm25SearchEngine {
    async fn index_document(
        &self,
        id: &str,
        path: &str,
        content: &str,
        kind: &str,
    ) -> anyhow::Result<()> {
        let mut writer = self.index.write().await;
        writer.index_document(id, path, content, kind);
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
        let reader = self.index.read().await;
        let query_terms = tokenize(query);
        Ok(reader.search(&query_terms, kind, limit))
    }

    async fn delete_document(&self, id: &str) -> anyhow::Result<()> {
        let mut writer = self.index.write().await;
        writer.delete_document(id);
        Ok(())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        let mut writer = self.index.write().await;
        writer.clear();
        Ok(())
    }

    async fn doc_count(&self) -> anyhow::Result<usize> {
        let reader = self.index.read().await;
        Ok(reader.documents.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Tokenization Tests (Target: >=2) ---

    #[test]
    fn test_tokenization_basic() {
        let tokens = tokenize("Rust language is fast!");
        assert_eq!(tokens, vec!["rust", "language", "is", "fast"]);
    }

    #[test]
    fn test_tokenization_punctuation_and_whitespace() {
        let tokens = tokenize("Hello, world!!!   Are you   READY??");
        assert_eq!(tokens, vec!["hello", "world", "are", "you", "ready"]);
    }

    // --- BM25 Scoring Tests (Target: >=4) ---

    #[test]
    fn test_bm25_empty_index() {
        let index = Bm25Index::new();
        let query_terms = tokenize("rust");
        let results = index.search(&query_terms, None, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_relevance_ranking() {
        let mut index = Bm25Index::new();
        // Index documents with varying term frequencies
        index.index_document(
            "1",
            "doc1.md",
            "rust is systems programming language systems systems",
            "code",
        );
        index.index_document(
            "2",
            "doc2.md",
            "python is systems programming language",
            "code",
        );
        index.index_document("3", "doc3.md", "java is enterprise language", "code");

        let query_terms = tokenize("systems");
        let results = index.search(&query_terms, None, 10);

        // Expect doc1 to rank higher than doc2 because "systems" occurs more frequently in doc1.
        // Expect doc3 to have no match.
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "1");
        assert_eq!(results[1].id, "2");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_bm25_idf_effect() {
        let mut index = Bm25Index::new();
        // "common" is very common; "rare" is rare.
        index.index_document(
            "1",
            "doc1.md",
            "this contains a very rare and precious jewel",
            "text",
        );
        index.index_document("2", "doc2.md", "this is a very common sentence", "text");
        index.index_document("3", "doc3.md", "another very common thing indeed", "text");

        // Querying for "rare" should give a high score to doc1.
        // Querying for "very" should match doc1, doc2, and doc3, but with much lower score per match due to low IDF.
        let query_rare = tokenize("rare");
        let results_rare = index.search(&query_rare, None, 10);
        assert_eq!(results_rare.len(), 1);
        assert_eq!(results_rare[0].id, "1");

        let query_common = tokenize("very");
        let results_common = index.search(&query_common, None, 10);
        assert_eq!(results_common.len(), 3);

        // The score of matching a rare word "rare" (unique to 1 doc) should be significantly higher
        // than matching a highly common word "very" (present in all 3 docs) in doc 1.
        let rare_score = results_rare[0].score;
        let common_score_in_doc1 = results_common.iter().find(|r| r.id == "1").unwrap().score;
        assert!(rare_score > common_score_in_doc1);
    }

    #[test]
    fn test_bm25_document_length_normalization() {
        let mut index = Bm25Index::new();
        // doc1 is short and to the point.
        // doc2 has the same target term but is surrounded by massive amount of spam tokens (longer document).
        index.index_document("1", "short.md", "rust is awesome", "code");

        let spam = "spam ".repeat(100);
        index.index_document("2", "long.md", &format!("rust is awesome {}", spam), "code");

        let query = tokenize("awesome");
        let results = index.search(&query, None, 10);

        // Due to length normalization, the shorter document (doc1) should be scored higher.
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "1");
        assert_eq!(results[1].id, "2");
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn test_local_bm25_search_engine_trait() {
        let engine = LocalBm25SearchEngine::new();
        engine
            .index_document("1", "a.md", "rust is fast", "code")
            .await
            .unwrap();
        engine
            .index_document("2", "b.md", "python is easy", "code")
            .await
            .unwrap();

        assert_eq!(engine.doc_count().await.unwrap(), 2);

        let results = engine.search("rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");

        engine.delete_document("1").await.unwrap();
        assert_eq!(engine.doc_count().await.unwrap(), 1);

        engine.clear().await.unwrap();
        assert_eq!(engine.doc_count().await.unwrap(), 0);
    }
}
