use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub url: String,
    pub local_path: Option<String>,
}

/// A boundary port interface for managing, cloning, and listing repositories.
///
/// Implementations handle workspace isolation and coordinate filesystem operations
/// on cloned repositories.
#[async_trait]
pub trait RepoManager: Send + Sync {
    /// Clones a git repository located at the specified URL.
    ///
    /// # Parameters
    /// - `url`: The clone URL of the repository (e.g. HTTPS or SSH).
    ///
    /// # Returns
    /// - `Ok(Repository)` containing repository metadata and its localized path.
    /// - `Err` if the cloning fails or the URL is unreachable.
    async fn clone_repo(&self, url: &str) -> anyhow::Result<Repository>;

    /// Lists all repositories registered or accessible in the current environment.
    ///
    /// # Returns
    /// - `Ok(Vec<Repository>)` of accessible repositories.
    /// - `Err` on persistent database/VFS failure.
    async fn list_repos(&self) -> anyhow::Result<Vec<Repository>>;
}

/// A structured result representing a document or chunk retrieved from a vector search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredResult {
    /// The unique identifier of the stored record.
    pub id: String,
    /// The similarity score (higher is typically more relevant, often [0.0, 1.0]).
    pub score: f32,
    /// Associated metadata (like file path, workspace name, or checksum).
    pub metadata: serde_json::Value,
}

/// A boundary port interface for storing and retrieving high-dimensional vector embeddings.
///
/// It acts as the bridge to specialized vector databases like surrealdb (using cosine similarity index)
/// or Qdrant/Pinecone.
#[async_trait]
pub trait VectorDb: Send + Sync {
    /// Stores an embedding vector in the designated collection/table.
    ///
    /// # Parameters
    /// - `collection`: The target table or logical database namespace.
    /// - `id`: Unique record identifier within that collection.
    /// - `vector`: The high-dimensional float vector.
    /// - `metadata`: JSON payload carrying extra document/chunk details.
    async fn store_embedding(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()>;

    /// Searches for matching high-dimensional vectors.
    ///
    /// # Parameters
    /// - `collection`: The collection/table to search inside.
    /// - `vector`: The query embedding vector.
    /// - `limit`: The maximum number of scored matches to return.
    ///
    /// # Returns
    /// - `Ok(Vec<ScoredResult>)` of similar items ordered by descending relevance.
    /// - `Err` on connection or query execution failure.
    async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>>;
}
