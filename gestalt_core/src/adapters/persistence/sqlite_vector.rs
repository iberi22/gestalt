//! SQLite-backed implementation of the [`VectorDb`] outbound port.
//!
//! [`SqliteVectorDb`] is the persistence adapter that the application layer
//! injects wherever an `Arc<dyn VectorDb>` is expected. It owns the embedding
//! domain rules (input validation, error context) and delegates the actual
//! storage and similarity search to [`SqliteDb`], which owns the `embeddings`
//! table and the `sqlite-vec` integration.
//!
//! Splitting responsibilities this way keeps a single SQL/schema owner
//! ([`SqliteDb`]) while the adapter remains a thin, well-typed boundary that
//! can validate inputs and add diagnostics without touching the storage layer.
//!
//! # Storage layout
//!
//! Vectors are stored as `BLOB`s in `embeddings.vector`, one native-endian
//! `f32` per component (4 bytes each, little-endian on every target Gestalt
//! supports). This is the layout `sqlite-vec` expects: it `memcpy`s the blob
//! into an `f32` array before computing `vec_distance_cosine`.
//!
//! Metadata is stored as a JSON `TEXT` column and deserialized back into a
//! [`serde_json::Value`] on the way out.

use std::path::Path;

use anyhow::Context;
use async_trait::async_trait;
use serde_json::Value;

use crate::db::SqliteDb;
use crate::ports::outbound::repo_manager::{ScoredResult, VectorDb};

/// Byte width of a single serialized `f32` component.
pub const F32_BYTES: usize = std::mem::size_of::<f32>();

/// Serializes a float vector into the byte layout used by the `embeddings.vector`
/// column.
///
/// Each component is written as a native-endian `f32`, matching what
/// `sqlite-vec` does when it reinterprets the blob as a float array. All
/// supported targets are little-endian, so this is the canonical
/// "little-endian `f32` slice" wire format.
pub fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * F32_BYTES);
    for value in vector {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

/// Inverse of [`encode_vector`].
///
/// # Errors
///
/// Returns an error when `bytes` is not a whole number of `f32` components
/// (for example, a truncated or corrupted blob).
pub fn decode_vector(bytes: &[u8]) -> anyhow::Result<Vec<f32>> {
    let (chunks, remainder) = bytes.as_chunks::<F32_BYTES>();
    if !remainder.is_empty() {
        anyhow::bail!(
            "invalid embedding blob: {} bytes is not a multiple of {F32_BYTES}",
            bytes.len()
        );
    }

    Ok(chunks.iter().copied().map(f32::from_ne_bytes).collect())
}

/// Validates that `vector` is a usable embedding: non-empty and finite.
///
/// `NaN`/`inf` components are rejected because they poison cosine similarity
/// and silently rank every neighbour as `NaN`.
fn validate_vector(vector: &[f32]) -> anyhow::Result<()> {
    if vector.is_empty() {
        anyhow::bail!("embedding vector is empty");
    }
    if let Some(index) = vector.iter().position(|value| !value.is_finite()) {
        anyhow::bail!("embedding vector contains a non-finite value at index {index}");
    }
    Ok(())
}

/// Validates that a collection name or record id is not blank.
fn validate_label(value: &str, label: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        anyhow::bail!("{label} must not be empty");
    }
    Ok(())
}

/// A [`VectorDb`] adapter backed by the shared [`SqliteDb`] embedded database.
///
/// The adapter is cheap to clone (it wraps the pooled [`SqliteDb`]) and is the
/// recommended way to hand vector storage to the application layer:
///
/// ```no_run
/// use std::sync::Arc;
///
/// use gestalt_core::adapters::persistence::SqliteVectorDb;
/// use gestalt_core::ports::outbound::repo_manager::VectorDb;
///
/// # async fn run() -> anyhow::Result<()> {
/// let db: Arc<dyn VectorDb> = Arc::new(SqliteVectorDb::open_in_memory()?);
/// db.store_embedding("chunks", "chunk-1", vec![0.1, 0.2], serde_json::json!({}))
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SqliteVectorDb {
    db: SqliteDb,
}

impl SqliteVectorDb {
    /// Wraps an already-open [`SqliteDb`], sharing its connection pool.
    pub fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Opens (or creates) a SQLite database at `path` and wraps it.
    ///
    /// # Errors
    ///
    /// Propagates connection, migration, and `sqlite-vec` registration failures
    /// from [`SqliteDb::open`].
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self::new(SqliteDb::open(path)?))
    }

    /// Opens a fresh in-memory database, useful for tests and ephemeral work.
    ///
    /// # Errors
    ///
    /// Propagates connection, migration, and `sqlite-vec` registration failures
    /// from [`SqliteDb::open_in_memory`].
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Ok(Self::new(SqliteDb::open_in_memory()?))
    }

    /// Borrows the underlying database handle.
    pub fn inner(&self) -> &SqliteDb {
        &self.db
    }

    /// Consumes the adapter and returns the underlying database handle.
    pub fn into_inner(self) -> SqliteDb {
        self.db
    }
}

impl From<SqliteDb> for SqliteVectorDb {
    fn from(db: SqliteDb) -> Self {
        Self::new(db)
    }
}

impl From<SqliteVectorDb> for SqliteDb {
    fn from(adapter: SqliteVectorDb) -> Self {
        adapter.into_inner()
    }
}

impl std::fmt::Debug for SqliteVectorDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteVectorDb").finish_non_exhaustive()
    }
}

#[async_trait]
impl VectorDb for SqliteVectorDb {
    async fn store_embedding(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata: Value,
    ) -> anyhow::Result<()> {
        validate_label(collection, "collection")?;
        validate_label(id, "id")?;
        validate_vector(&vector)?;

        self.db
            .insert_embedding(collection, id, vector, metadata)
            .await
            .with_context(|| {
                format!("failed to store embedding {id:?} in collection {collection:?}")
            })
    }

    async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>> {
        validate_label(collection, "collection")?;
        validate_vector(&vector)?;

        // A zero limit can never return matches; skip the round-trip entirely.
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.db
            .search_similar(collection, vector, limit)
            .await
            .with_context(|| format!("failed to search collection {collection:?}"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn adapter() -> SqliteVectorDb {
        SqliteVectorDb::open_in_memory().expect("in-memory database")
    }

    fn trait_object(db: SqliteVectorDb) -> Arc<dyn VectorDb> {
        Arc::new(db)
    }

    #[test]
    fn encode_decode_roundtrip() {
        let vector = vec![0.0, 1.0, -1.5, f32::MIN_POSITIVE, 1.0e-6];
        let bytes = encode_vector(&vector);

        assert_eq!(bytes.len(), vector.len() * F32_BYTES);
        assert_eq!(bytes[..F32_BYTES], 0.0f32.to_ne_bytes());
        assert_eq!(decode_vector(&bytes).unwrap(), vector);
    }

    #[test]
    fn encode_handles_empty_input() {
        assert!(encode_vector(&[]).is_empty());
        assert_eq!(decode_vector(&[]).unwrap(), Vec::<f32>::new());
    }

    #[test]
    fn decode_rejects_truncated_blob() {
        let error = decode_vector(&[0x00, 0x01, 0x02]).unwrap_err();
        assert!(
            error.to_string().contains("not a multiple"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn stores_and_searches_through_the_port() {
        let db = trait_object(adapter());

        db.store_embedding(
            "chunks",
            "a",
            vec![1.0, 0.0, 0.0],
            serde_json::json!({ "path": "a.rs" }),
        )
        .await
        .unwrap();
        db.store_embedding(
            "chunks",
            "b",
            vec![0.0, 1.0, 0.0],
            serde_json::json!({ "path": "b.rs" }),
        )
        .await
        .unwrap();

        let hits = db
            .search_similar("chunks", vec![1.0, 0.0, 0.0], 10)
            .await
            .unwrap();

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "a");
        assert!(hits[0].score > 0.99, "score was {}", hits[0].score);
        assert_eq!(hits[0].metadata["path"], "a.rs");
        assert_eq!(hits[1].id, "b");
        assert!(hits[1].score.abs() < 0.01, "score was {}", hits[1].score);
    }

    #[tokio::test]
    async fn store_overwrites_existing_record() {
        let db = adapter();
        db.store_embedding("c", "id", vec![1.0, 0.0], serde_json::json!({ "v": 1 }))
            .await
            .unwrap();
        db.store_embedding("c", "id", vec![0.0, 1.0], serde_json::json!({ "v": 2 }))
            .await
            .unwrap();

        let hits = db.search_similar("c", vec![0.0, 1.0], 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "id");
        assert_eq!(hits[0].metadata["v"], 2);
    }

    #[tokio::test]
    async fn search_orders_by_similarity_and_respects_limit() {
        let db = adapter();
        db.store_embedding("c", "exact", vec![1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        db.store_embedding("c", "near", vec![0.9, 0.1], serde_json::json!({}))
            .await
            .unwrap();
        db.store_embedding("c", "far", vec![0.0, 1.0], serde_json::json!({}))
            .await
            .unwrap();

        let all = db.search_similar("c", vec![1.0, 0.0], 10).await.unwrap();
        let ids: Vec<&str> = all.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(ids, vec!["exact", "near", "far"]);

        let limited = db.search_similar("c", vec![1.0, 0.0], 2).await.unwrap();
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].id, "exact");
    }

    #[tokio::test]
    async fn collections_are_isolated() {
        let db = adapter();
        db.store_embedding("c1", "x", vec![1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        db.store_embedding("c2", "y", vec![1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();

        let hits = db.search_similar("c2", vec![1.0, 0.0], 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "y");
    }

    #[tokio::test]
    async fn search_skips_vectors_with_a_different_width() {
        let db = adapter();
        db.store_embedding("mixed", "two", vec![1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        db.store_embedding("mixed", "three", vec![1.0, 0.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();

        // Dimension mismatches must not abort the whole search.
        let hits = db
            .search_similar("mixed", vec![1.0, 0.0, 0.0], 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "three");
    }

    #[tokio::test]
    async fn search_on_unknown_collection_is_empty() {
        let db = adapter();
        let hits = db
            .search_similar("missing", vec![1.0, 0.0], 5)
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn zero_limit_short_circuits() {
        let db = adapter();
        db.store_embedding("c", "a", vec![1.0], serde_json::json!({}))
            .await
            .unwrap();
        assert!(db
            .search_similar("c", vec![1.0], 0)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn rejects_empty_vectors() {
        let db = adapter();

        let store_error = db
            .store_embedding("c", "a", Vec::new(), serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(store_error.to_string().contains("empty"));

        let search_error = db.search_similar("c", Vec::new(), 1).await.unwrap_err();
        assert!(search_error.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn rejects_non_finite_components() {
        let db = adapter();

        let nan_error = db
            .store_embedding("c", "a", vec![1.0, f32::NAN], serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(nan_error.to_string().contains("non-finite"), "{nan_error}");

        let inf_error = db
            .search_similar("c", vec![f32::INFINITY], 1)
            .await
            .unwrap_err();
        assert!(inf_error.to_string().contains("non-finite"), "{inf_error}");
    }

    #[tokio::test]
    async fn rejects_blank_collection_or_id() {
        let db = adapter();

        let blank_collection = db
            .store_embedding("  ", "a", vec![1.0], serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(blank_collection.to_string().contains("collection"));

        let blank_id = db
            .store_embedding("c", "", vec![1.0], serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(blank_id.to_string().contains("id"));

        let blank_search = db.search_similar("", vec![1.0], 1).await.unwrap_err();
        assert!(blank_search.to_string().contains("collection"));
    }

    #[tokio::test]
    async fn wraps_up_an_existing_database() {
        let db = SqliteDb::open_in_memory().unwrap();
        let adapter = SqliteVectorDb::from(db);

        adapter
            .store_embedding("c", "a", vec![1.0, 1.0], serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(adapter.inner().list_repos().await.unwrap().len(), 0);

        // Round-trip through the conversion impls.
        let inner: SqliteDb = adapter.into_inner();
        assert!(inner.list_repos().await.unwrap().is_empty());
    }

    #[test]
    fn debug_impl_does_not_leak_connection_details() {
        let rendered = format!("{:?}", adapter());
        assert!(rendered.starts_with("SqliteVectorDb"));
    }
}
