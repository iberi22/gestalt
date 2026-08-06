//! Hybrid Search Fusion using Reciprocal Rank Fusion (RRF).
//!
//! Provides the logic to combine lexical (BM25) and dense (Vector) search results
//! into a single, unified ranked list using the Reciprocal Rank Fusion algorithm.

use crate::ports::outbound::search::SearchResult;
use std::collections::HashMap;

/// Reciprocal Rank Fusion (RRF) constant parameter (k), typically set to 60.0.
const RRF_K: f64 = 60.0;

/// Fuses BM25 and Vector search results using the Reciprocal Rank Fusion (RRF) algorithm.
///
/// # Arguments
/// * `_query` — The search query (not strictly used for RRF, but part of signature).
/// * `vector_results` — Results from vector similarity search.
/// * `bm25_results` — Results from BM25 lexical search.
///
/// # Returns
/// A sorted vector of [`SearchResult`] ranked by their RRF score descending.
pub fn hybrid_search(
    _query: &str,
    vector_results: &[SearchResult],
    bm25_results: &[SearchResult],
) -> Vec<SearchResult> {
    if vector_results.is_empty() && bm25_results.is_empty() {
        return Vec::new();
    }

    // Map doc ID -> (RRF score, SearchResult template)
    let mut fused: HashMap<String, (f64, SearchResult)> = HashMap::new();

    // 1. Process vector results
    for (rank_idx, item) in vector_results.iter().enumerate() {
        let rank = (rank_idx + 1) as f64;
        let rrf_contribution = 1.0 / (RRF_K + rank);

        fused
            .entry(item.id.clone())
            .and_modify(|(score, _)| *score += rrf_contribution)
            .or_insert_with(|| (rrf_contribution, item.clone()));
    }

    // 2. Process BM25 results
    for (rank_idx, item) in bm25_results.iter().enumerate() {
        let rank = (rank_idx + 1) as f64;
        let rrf_contribution = 1.0 / (RRF_K + rank);

        fused
            .entry(item.id.clone())
            .and_modify(|(score, existing)| {
                *score += rrf_contribution;
                // Prefer the metadata/snippets if BM25 results are more comprehensive
                // or just keep existing. Let's make sure we preserve the best fields.
                if existing.snippet.is_empty() && !item.snippet.is_empty() {
                    existing.snippet = item.snippet.clone();
                }
            })
            .or_insert_with(|| (rrf_contribution, item.clone()));
    }

    // 3. Convert to a sorted Vec of SearchResult
    let mut results: Vec<SearchResult> = fused
        .into_values()
        .map(|(rrf_score, mut item)| {
            item.score = rrf_score;
            item
        })
        .collect();

    // Sort descending by RRF score
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_result(id: &str, score: f64) -> SearchResult {
        SearchResult {
            id: id.to_string(),
            path: format!("{}.md", id),
            content: format!("Content of {}", id),
            snippet: format!("Snippet of {}", id),
            score,
        }
    }

    // --- RRF Fusion Tests (Target: >=4) ---

    #[test]
    fn test_rrf_empty_inputs() {
        let results = hybrid_search("test", &[], &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rrf_single_input() {
        let vector = vec![mock_result("A", 0.9), mock_result("B", 0.8)];
        let results = hybrid_search("test", &vector, &[]);

        // Should preserve the order: A then B
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "A");
        assert_eq!(results[1].id, "B");
        // RRF scores:
        // A: 1 / (60 + 1) = 0.01639344...
        // B: 1 / (60 + 2) = 0.01612903...
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_rrf_merging_and_ranking() {
        // Document A is ranked first in vector, but absent in BM25.
        // Document B is ranked second in vector, but first in BM25.
        // Because B is present in both, its cumulative RRF score should beat A.
        let vector = vec![mock_result("A", 0.9), mock_result("B", 0.8)];
        let bm25 = vec![mock_result("B", 15.0), mock_result("C", 10.0)];

        let results = hybrid_search("test", &vector, &bm25);

        assert_eq!(results.len(), 3);

        // Let's compute RRF scores:
        // A: Rank 1 in vector -> 1 / (60 + 1) = 0.016393
        // B: Rank 2 in vector -> 1 / (60 + 2) = 0.016129
        //    Rank 1 in BM25   -> 1 / (60 + 1) = 0.016393
        //    Total B = 0.032522
        // C: Rank 2 in BM25   -> 1 / (60 + 2) = 0.016129
        //
        // Sorted order should be: B, A, C
        assert_eq!(results[0].id, "B");
        assert_eq!(results[1].id, "A");
        assert_eq!(results[2].id, "C");

        assert!(results[0].score > results[1].score);
        assert!(results[1].score > results[2].score);
    }

    #[test]
    fn test_rrf_non_overlapping() {
        let vector = vec![mock_result("A", 0.9), mock_result("B", 0.8)];
        let bm25 = vec![mock_result("C", 12.0), mock_result("D", 8.0)];

        let results = hybrid_search("test", &vector, &bm25);

        assert_eq!(results.len(), 4);
        // Scores:
        // A: Rank 1 -> 1 / 61
        // B: Rank 2 -> 1 / 62
        // C: Rank 1 -> 1 / 61
        // D: Rank 2 -> 1 / 62
        // A and C should tie. B and D should tie.
        let score_rank1 = 1.0 / 61.0;
        let score_rank2 = 1.0 / 62.0;

        assert!((results[0].score - score_rank1).abs() < 1e-9);
        assert!((results[1].score - score_rank1).abs() < 1e-9);
        assert!((results[2].score - score_rank2).abs() < 1e-9);
        assert!((results[3].score - score_rank2).abs() < 1e-9);
    }
}
