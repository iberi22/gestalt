use crate::ports::outbound::repo_manager::VectorDb;
use async_trait::async_trait;
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
            .create((collection, id))
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
        _vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        // SurrealDB 1.x doesn't have native vector search, use simple query for now
        let mut response = self
            .db
            .query("SELECT * FROM type::table($table) LIMIT $limit")
            .bind(("table", collection.to_string()))
            .bind(("limit", limit))
            .await?;

        let results: Vec<serde_json::Value> = response.take(0)?;
        Ok(results)
    }
}
