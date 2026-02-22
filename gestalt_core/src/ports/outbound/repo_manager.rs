use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub url: String,
    pub local_path: Option<String>,
}

#[async_trait]
pub trait RepoManager: Send + Sync {
    async fn clone_repo(&self, url: &str) -> anyhow::Result<Repository>;
    async fn list_repos(&self) -> anyhow::Result<Vec<Repository>>;
}

#[async_trait]
pub trait VectorDb: Send + Sync {
    async fn store_embedding(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()>;
    async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>>;
}
