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
        let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
        db.use_ns("neural").use_db("link").await?;
        Ok(Self { db })
    }

    fn sanitize_table_name(collection: &str) -> anyhow::Result<String> {
        if collection.is_empty() {
            return Err(anyhow::anyhow!("Collection name cannot be empty"));
        }

        if !collection
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(anyhow::anyhow!(
                "Invalid collection name '{}': only [a-zA-Z0-9_] are allowed",
                collection
            ));
        }

        Ok(collection.to_string())
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
        let table = Self::sanitize_table_name(collection)?;
        self.db
            .query(
                "UPSERT type::thing($table, $id) CONTENT { embedding: $embedding, metadata: $metadata }",
            )
            .bind(("table", table))
            .bind(("id", id.to_string()))
            .bind(("embedding", vector))
            .bind(("metadata", metadata))
            .await?;
        Ok(())
    }

    async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>> {
        let table = Self::sanitize_table_name(collection)?;
        let query = format!(
            "SELECT id, metadata, vector::distance::cosine(embedding, $vector) AS score \
             FROM {} WHERE embedding IS NOT NONE ORDER BY score ASC LIMIT $limit",
            table
        );

        let mut response = self
            .db
            .query(query)
            .bind(("vector", vector))
            .bind(("limit", limit))
            .await?;

        let results: Vec<SearchResult> = response.take(0)?;

        if results.is_empty() {
            // Simple lexical fallback: return existing records when similarity returns empty.
            let fallback_query = format!("SELECT id, metadata FROM {} LIMIT $limit", table);

            let mut response = self.db.query(fallback_query).bind(("limit", limit)).await?;
            let fallback_results: Vec<VectorRecord> = response.take(0)?;
            return Ok(fallback_results
                .into_iter()
                .map(|r| ScoredResult {
                    id: r.id.to_string(),
                    score: 0.0,
                    metadata: r.metadata,
                })
                .collect());
        }

        Ok(results
            .into_iter()
            .map(|r| ScoredResult {
                id: r.id.to_string(),
                score: 1.0 - r.score, // Convert distance to similarity
                metadata: r.metadata,
            })
            .collect())
    }
}
