use anyhow::Result;
use chrono::Utc;
use gestalt_core::application::indexer::{Indexer, RepositoryMetadata, DocumentRecord};
use crate::db::SurrealClient;
use tracing::info;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepoRecord {
    id: Option<Thing>,
    url: String,
    name: String,
    local_path: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocRecord {
    id: Option<Thing>,
    repo_id: String,
    path: String,
    checksum: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkRecord {
    doc_id: String,
    content: String,
    chunk_index: usize,
    created_at: chrono::DateTime<Utc>,
}

pub struct IndexService {
    db: SurrealClient,
    indexer: Indexer,
}

impl IndexService {
    pub fn new(db: SurrealClient) -> Self {
        Self {
            db,
            indexer: Indexer::default(),
        }
    }

    pub async fn index_repo(&self, url: &str) -> Result<()> {
        let (repo_meta, path, _temp_dir) = self.indexer.ingest(url).await?;

        // 1. Ensure repository exists in DB
        let repo_id = self.ensure_repo(&repo_meta).await?;

        // 2. Scan files
        let files = self.indexer.scan(&path);
        info!("Found {} files to index in {}", files.len(), url);

        for file_path in files {
            let record = self.indexer.process_file(&path, &file_path)?;

            // 3. Check checksum and update/skip
            if let Some(doc_id) = self.should_update_doc(&repo_id, &record).await? {
                info!("Indexing file: {}", record.metadata.path);
                self.persist_doc(&doc_id, record).await?;
            } else {
                info!("Skipping unchanged file: {}", record.metadata.path);
            }
        }

        Ok(())
    }

    async fn ensure_repo(&self, meta: &RepositoryMetadata) -> Result<String> {
        let query = "SELECT * FROM repositories WHERE url = $url";
        let existing: Vec<RepoRecord> = self.db.query_with(query, serde_json::json!({ "url": meta.url })).await?;

        if let Some(repo) = existing.first() {
            Ok(repo.id.as_ref().unwrap().to_string())
        } else {
            let new_repo = RepoRecord {
                id: None,
                url: meta.url.clone(),
                name: meta.name.clone(),
                local_path: meta.local_path.clone(),
                created_at: Utc::now(),
            };
            let created: RepoRecord = self.db.create("repositories", &new_repo).await?;
            Ok(created.id.as_ref().unwrap().to_string())
        }
    }

    async fn should_update_doc(&self, repo_id: &str, record: &DocumentRecord) -> Result<Option<String>> {
        let query = "SELECT * FROM documents WHERE repo_id = $repo_id AND path = $path";
        let existing: Vec<DocRecord> = self.db.query_with(query, serde_json::json!({
            "repo_id": repo_id,
            "path": record.metadata.path
        })).await?;

        if let Some(doc) = existing.first() {
            if doc.checksum == record.metadata.checksum {
                Ok(None) // No update needed
            } else {
                Ok(Some(doc.id.as_ref().unwrap().to_string())) // Needs update
            }
        } else {
            // New document
            let new_doc = DocRecord {
                id: None,
                repo_id: repo_id.to_string(),
                path: record.metadata.path.clone(),
                checksum: record.metadata.checksum.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            let created: DocRecord = self.db.create("documents", &new_doc).await?;
            Ok(Some(created.id.as_ref().unwrap().to_string()))
        }
    }

    async fn persist_doc(&self, doc_id: &str, record: DocumentRecord) -> Result<()> {
        // 1. Update document checksum and updated_at
        let _: Vec<serde_json::Value> = self.db.query_with(
            "UPDATE type::thing($doc_id) SET checksum = $checksum, updated_at = $now",
            serde_json::json!({
                "doc_id": doc_id,
                "checksum": record.metadata.checksum,
                "now": Utc::now()
            })
        ).await?;

        // 2. Delete old chunks
        let _: Vec<serde_json::Value> = self.db.query_with(
            "DELETE chunks WHERE doc_id = $doc_id",
            serde_json::json!({ "doc_id": doc_id })
        ).await?;

        // 3. Insert new chunks
        for chunk in record.chunks {
            let chunk_record = ChunkRecord {
                doc_id: doc_id.to_string(),
                content: chunk.content,
                chunk_index: chunk.index,
                created_at: Utc::now(),
            };
            self.db.create("chunks", &chunk_record).await?;
        }

        Ok(())
    }
}
