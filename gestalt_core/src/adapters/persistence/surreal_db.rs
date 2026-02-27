use crate::ports::outbound::repo_manager::{ScoredResult, VectorDb};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

pub struct SurrealDbAdapter {
    db: Surreal<Db>,
}

impl SurrealDbAdapter {
    pub async fn new() -> anyhow::Result<Self> {
        let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
        db.use_ns("neural").use_db("link").await?;
        Ok(Self { db })
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct VectorRecord {
    id: surrealdb::sql::Thing,
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    id: surrealdb::sql::Thing,
    score: f32,
    metadata: serde_json::Value,
}

#[async_trait]
impl VectorDb for SurrealDbAdapter {
    async fn store_embedding(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()> {
        let _: Option<serde_json::Value> = self
            .db
            .upsert((collection, id))
            .content(serde_json::json!({
                "embedding": vector,
                "metadata": metadata
            }))
            .await?;
        Ok(())
    }

    async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>> {
        let mut response = self
            .db
            .query("SELECT id, metadata, vector::distance::cosine(embedding, $vector) AS score FROM type::table($table) ORDER BY score ASC LIMIT $limit")
            .bind(("vector", vector))
            .bind(("table", collection.to_string()))
            .bind(("limit", limit))
            .await?;

        let results: Vec<SearchResult> = response.take(0)?;

        if results.is_empty() {
             // Simple lexical fallback: just return some records if no similarity found or embeddings missing
             // In a real scenario, this would be a separate full-text search index query
             let mut response = self
                .db
                .query("SELECT id, metadata FROM type::table($table) LIMIT $limit")
                .bind(("table", collection.to_string()))
                .bind(("limit", limit))
                .await?;
             let fallback_results: Vec<VectorRecord> = response.take(0)?;
             return Ok(fallback_results.into_iter().map(|r| ScoredResult {
                id: r.id.to_string(),
                score: 0.0,
                metadata: r.metadata,
             }).collect());
        }

        Ok(results.into_iter().map(|r| ScoredResult {
            id: r.id.to_string(),
            score: 1.0 - r.score, // Convert distance to similarity
            metadata: r.metadata,
        }).collect())
    }
}
