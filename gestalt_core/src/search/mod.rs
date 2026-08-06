//! Local and Hybrid Search module.
//!
//! Exposes in-memory BM25 index and Reciprocal Rank Fusion (RRF) search utilities.

pub mod bm25;
pub mod fusion;

pub use bm25::{tokenize, Bm25Document, Bm25Index, LocalBm25SearchEngine};
pub use fusion::hybrid_search;
