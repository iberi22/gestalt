//! SurrealDB client implementation

use crate::config::DatabaseSettings;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::info;

/// SurrealDB client wrapper.
#[derive(Clone)]
pub struct SurrealClient {
    db: Arc<Surreal<Any>>,
}

impl SurrealClient {
    /// Connect to SurrealDB using provided settings.
    pub async fn connect(config: &DatabaseSettings) -> Result<Self> {
        info!("Connecting to SurrealDB at {}", config.url);

        let db = surrealdb::engine::any::connect(&config.url)
            .await
            .context("Failed to connect to SurrealDB")?;

        // Only sign in for remote connections
        if config.url.starts_with("ws://")
            || config.url.starts_with("wss://")
            || config.url.starts_with("http://")
            || config.url.starts_with("https://")
        {
            db.signin(Root {
                username: &config.user,
                password: &config.pass,
            })
            .await
            .context("Failed to authenticate with SurrealDB")?;
        }

        db.use_ns(&config.namespace)
            .use_db(&config.database)
            .await?;

        info!(
            "Connected to SurrealDB: {}:{}",
            config.namespace, config.database
        );

        Ok(Self { db: Arc::new(db) })
    }

    /// Create a record in a table.
    pub async fn create<T: Serialize + DeserializeOwned>(
        &self,
        table: &str,
        data: &T,
    ) -> Result<T> {
        let val = serde_json::to_value(data)?;
        let result: Option<T> = self.db.create(table).content(val).await?;
        result.ok_or_else(|| anyhow::anyhow!("Failed to create record"))
    }

    /// Select all records from a table.
    pub async fn select_all<T: DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        let results: Vec<T> = self.db.select(table).await?;
        Ok(results)
    }

    /// Select a record by ID.
    pub async fn select_by_id<T: DeserializeOwned>(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<T>> {
        let result: Option<T> = self.db.select((table, id)).await?;
        Ok(result)
    }

    /// Update a record.
    pub async fn update<T: Serialize + DeserializeOwned>(
        &self,
        table: &str,
        id: &str,
        data: &T,
    ) -> Result<T> {
        let val = serde_json::to_value(data)?;
        let result: Option<T> = self.db.update((table, id)).content(val).await?;
        result.ok_or_else(|| anyhow::anyhow!("Failed to update record"))
    }

    /// Upsert a record by ID.
    pub async fn upsert<T: Serialize + DeserializeOwned>(
        &self,
        table: &str,
        id: &str,
        data: &T,
    ) -> Result<T> {
        let val = serde_json::to_value(data)?;
        let table_name = table.to_string();
        let record_id = id.to_string();
        let mut response = self
            .db
            .query("UPSERT type::thing($table, $id) CONTENT $data RETURN AFTER")
            .bind(("table", table_name))
            .bind(("id", record_id))
            .bind(("data", val))
            .await?;
        let result: Option<T> = response.take(0)?;
        result.ok_or_else(|| anyhow::anyhow!("Failed to upsert record"))
    }

    /// Execute a raw query.
    pub async fn query(&self, query: &str) -> Result<surrealdb::Response> {
        let result = self.db.query(query).await?;
        Ok(result)
    }

    /// Execute a query with bindings.
    pub async fn query_with<T: DeserializeOwned>(
        &self,
        query: &str,
        bindings: impl Serialize,
    ) -> Result<Vec<T>> {
        let val = serde_json::to_value(bindings)?;
        let mut response = self.db.query(query).bind(val).await?;
        let results: Vec<T> = response.take(0)?;
        Ok(results)
    }
}
