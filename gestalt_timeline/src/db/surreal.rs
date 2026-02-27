//! SurrealDB client implementation

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::{debug, info};

/// SurrealDB client wrapper for timeline operations.
#[derive(Clone)]
pub struct SurrealClient {
    db: Arc<Surreal<Any>>,
}

impl SurrealClient {
    /// Connect to SurrealDB using provided settings.
    pub async fn connect(config: &crate::config::DatabaseSettings) -> Result<Self> {
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

        // Initialize schema
        Self::init_schema(&db).await?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Initialize database schema.
    async fn init_schema(db: &Surreal<Any>) -> Result<()> {
        debug!("Initializing database schema");

        // Create tables with indexes
        db.query(
            r#"
            DEFINE TABLE timeline_events SCHEMAFULL;
            DEFINE FIELD timestamp ON timeline_events TYPE any;
            DEFINE FIELD agent_id ON timeline_events TYPE string;
            DEFINE FIELD event_type ON timeline_events TYPE string;
            DEFINE FIELD project_id ON timeline_events TYPE option<string>;
            DEFINE FIELD task_id ON timeline_events TYPE option<string>;
            DEFINE FIELD payload ON timeline_events TYPE option<object>;
            DEFINE FIELD metadata ON timeline_events TYPE option<object>;
            DEFINE INDEX idx_timestamp ON timeline_events FIELDS timestamp;
            DEFINE INDEX idx_project ON timeline_events FIELDS project_id;
            DEFINE INDEX idx_agent ON timeline_events FIELDS agent_id;

            DEFINE TABLE projects SCHEMAFULL;
            DEFINE FIELD name ON projects TYPE string;
            DEFINE FIELD status ON projects TYPE string;
            DEFINE FIELD priority ON projects TYPE int;
            DEFINE FIELD created_at ON projects TYPE any;
            DEFINE FIELD updated_at ON projects TYPE any;
            DEFINE FIELD created_by ON projects TYPE string;
            DEFINE INDEX idx_name ON projects FIELDS name UNIQUE;

            DEFINE TABLE tasks SCHEMAFULL;
            DEFINE FIELD project_id ON tasks TYPE string;
            DEFINE FIELD description ON tasks TYPE string;
            DEFINE FIELD status ON tasks TYPE string;
            DEFINE FIELD created_at ON tasks TYPE any;
            DEFINE FIELD updated_at ON tasks TYPE any;
            DEFINE FIELD completed_at ON tasks TYPE any;
            DEFINE FIELD created_by ON tasks TYPE string;
            DEFINE FIELD executed_by ON tasks TYPE option<string>;
            DEFINE FIELD duration_ms ON tasks TYPE option<int>;
            DEFINE INDEX idx_project_id ON tasks FIELDS project_id;
            DEFINE INDEX idx_status ON tasks FIELDS status;

            DEFINE TABLE agents SCHEMAFULL;
            DEFINE FIELD name ON agents TYPE string;
            DEFINE FIELD agent_type ON agents TYPE string;
            DEFINE FIELD status ON agents TYPE string;
            DEFINE FIELD connected_at ON agents TYPE datetime;
            DEFINE FIELD last_seen ON agents TYPE datetime;
            DEFINE FIELD command_count ON agents TYPE int;
            DEFINE FIELD system_prompt ON agents TYPE option<string>;
            DEFINE FIELD model_id ON agents TYPE option<string>;

            DEFINE TABLE agent_runtime_states SCHEMAFULL;
            DEFINE FIELD agent_id ON agent_runtime_states TYPE string;
            DEFINE FIELD goal ON agent_runtime_states TYPE string;
            DEFINE FIELD phase ON agent_runtime_states TYPE string;
            DEFINE FIELD current_step ON agent_runtime_states TYPE int;
            DEFINE FIELD max_steps ON agent_runtime_states TYPE int;
            DEFINE FIELD last_action ON agent_runtime_states TYPE option<string>;
            DEFINE FIELD last_observation ON agent_runtime_states TYPE option<string>;
            DEFINE FIELD history_tail ON agent_runtime_states TYPE array;
            DEFINE FIELD error ON agent_runtime_states TYPE option<string>;
            DEFINE FIELD started_at ON agent_runtime_states TYPE datetime;
            DEFINE FIELD updated_at ON agent_runtime_states TYPE datetime;
            DEFINE FIELD finished_at ON agent_runtime_states TYPE option<datetime>;
            DEFINE INDEX idx_runtime_agent ON agent_runtime_states FIELDS agent_id UNIQUE;

            DEFINE TABLE repositories SCHEMAFULL;
            DEFINE FIELD url ON repositories TYPE string;
            DEFINE FIELD name ON repositories TYPE string;
            DEFINE FIELD local_path ON repositories TYPE option<string>;
            DEFINE FIELD created_at ON repositories TYPE datetime;
            DEFINE INDEX idx_repo_url ON repositories FIELDS url UNIQUE;

            DEFINE TABLE documents SCHEMAFULL;
            DEFINE FIELD repo_id ON documents TYPE string;
            DEFINE FIELD path ON documents TYPE string;
            DEFINE FIELD checksum ON documents TYPE string;
            DEFINE FIELD created_at ON documents TYPE datetime;
            DEFINE FIELD updated_at ON documents TYPE datetime;
            DEFINE INDEX idx_doc_path ON documents FIELDS [repo_id, path] UNIQUE;

            DEFINE TABLE chunks SCHEMAFULL;
            DEFINE FIELD doc_id ON chunks TYPE string;
            DEFINE FIELD content ON chunks TYPE string;
            DEFINE FIELD chunk_index ON chunks TYPE int;
            DEFINE FIELD created_at ON chunks TYPE datetime;
            DEFINE INDEX idx_chunk_doc ON chunks FIELDS doc_id;
            "#,
        )
        .await
        .context("Failed to initialize schema")?;

        debug!("Schema initialized successfully");
        Ok(())
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

    /// Access the underlying Surreal client.
    pub fn client(&self) -> Arc<Surreal<Any>> {
        self.db.clone()
    }
    /// Delete a record.
    pub async fn delete(&self, table: &str, id: &str) -> Result<()> {
        let _: Option<serde_json::Value> = self.db.delete((table, id)).await?;
        Ok(())
    }
}
