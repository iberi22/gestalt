use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use walkdir::WalkDir;
use anyhow::Result;
use tracing::{info, warn};
use tempfile::tempdir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadata {
    pub url: String,
    pub name: String,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub content: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub metadata: FileMetadata,
    pub chunks: Vec<Chunk>,
}

pub struct Indexer {
    allowlist: Vec<String>,
    max_file_size: u64,
    chunk_size: usize,
    chunk_overlap: usize,
}

impl Default for Indexer {
    fn default() -> Self {
        Self {
            allowlist: vec![
                "rs".to_string(),
                "md".to_string(),
                "toml".to_string(),
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "json".to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
            chunk_size: 1000,
            chunk_overlap: 200,
        }
    }
}

impl Indexer {
    pub fn new(allowlist: Vec<String>, max_file_size: u64, chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            allowlist,
            max_file_size,
            chunk_size,
            chunk_overlap,
        }
    }

    /// Ingest a repository from a URL (local path or remote git URL).
    /// If remote, it clones to a temporary directory.
    pub async fn ingest(&self, url: &str) -> Result<(RepositoryMetadata, PathBuf, Option<tempfile::TempDir>)> {
        if Path::new(url).exists() {
            let path = PathBuf::from(url).canonicalize()?;
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            info!("Ingesting local repository: {} at {:?}", name, path);
            Ok((
                RepositoryMetadata {
                    url: url.to_string(),
                    name,
                    local_path: Some(path.to_string_lossy().to_string()),
                },
                path,
                None,
            ))
        } else {
            let temp_dir = tempdir()?;
            let path = temp_dir.path().to_path_buf();
            info!("Cloning remote repository: {} to {:?}", url, path);

            git2::Repository::clone(url, &path)?;

            let name = url.split('/').next_back().unwrap_or("unknown").trim_end_matches(".git").to_string();

            Ok((
                RepositoryMetadata {
                    url: url.to_string(),
                    name,
                    local_path: None,
                },
                path,
                Some(temp_dir),
            ))
        }
    }

    /// Scan the repository for relevant files.
    pub fn scan(&self, root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path();
                let relative = path.strip_prefix(root).unwrap_or(path);

                // Skip hidden directories (like .git)
                if relative
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
                {
                    continue;
                }

                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if self.allowlist.contains(&ext.to_string()) {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.len() <= self.max_file_size {
                                files.push(path.to_path_buf());
                            } else {
                                warn!("Skipping file {:?} (size {} exceeds limit {})", path, metadata.len(), self.max_file_size);
                            }
                        }
                    }
                }
            }
        }
        files
    }

    /// Process a file into chunks and metadata.
    pub fn process_file(&self, root: &Path, file_path: &Path) -> Result<DocumentRecord> {
        let relative_path = file_path.strip_prefix(root)?.to_string_lossy().to_string();
        let content = std::fs::read_to_string(file_path)?;

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let checksum = format!("{:x}", hasher.finalize());

        let chunks = self.chunk_text(&content);

        Ok(DocumentRecord {
            metadata: FileMetadata {
                path: relative_path,
                checksum,
            },
            chunks,
        })
    }

    fn chunk_text(&self, text: &str) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0;

        while start < chars.len() {
            let end = (start + self.chunk_size).min(chars.len());
            let chunk_content: String = chars[start..end].iter().collect();

            chunks.push(Chunk {
                content: chunk_content,
                index: chunks.len(),
            });

            if end == chars.len() {
                break;
            }

            start += self.chunk_size - self.chunk_overlap;
        }

        chunks
    }
}

use crate::application::agent::tools::SearchCodeTool;
use surrealdb::sql::Thing as RecordId;

#[async_trait::async_trait]
pub trait VectorAdapter: Send + Sync {
    async fn index_document(&self, repo_id: &str, doc: DocumentRecord) -> Result<()>;
    async fn search(&self, repo_id: &str, query: &str, limit: usize) -> Result<Vec<VectorRecord>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub path: String,
    pub content: String,
    pub score: f32,
}

pub struct SurrealAdapter {
    client: crate::db::surreal::SurrealClient,
}

impl SurrealAdapter {
    pub fn new(client: crate::db::surreal::SurrealClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl VectorAdapter for SurrealAdapter {
    async fn index_document(&self, repo_id: &str, doc: DocumentRecord) -> Result<()> {
        let created_at = chrono::Utc::now().to_rfc3339();

        // 1. Create document record
        let doc_id = format!("{}:{}", repo_id, doc.metadata.path);
        let _doc: serde_json::Value = self.client.upsert(
            "documents",
            &doc_id,
            &serde_json::json!({
                "repo_id": repo_id,
                "path": doc.metadata.path,
                "checksum": doc.metadata.checksum,
                "created_at": created_at,
                "updated_at": created_at,
            })
        ).await?;

        // 2. Create chunk records
        for chunk in doc.chunks {
            let chunk_id = format!("{}:{}", doc_id, chunk.index);
            let _chunk: serde_json::Value = self.client.upsert(
                "chunks",
                &chunk_id,
                &serde_json::json!({
                    "doc_id": doc_id,
                    "content": chunk.content,
                    "chunk_index": chunk.index,
                    "created_at": created_at,
                })
            ).await?;
        }

        Ok(())
    }

    async fn search(&self, repo_id: &str, query: &str, limit: usize) -> Result<Vec<VectorRecord>> {
        // Simple keyword-based search in SurrealDB as a fallback for pure vector search
        let sql = "SELECT path, content FROM chunks WHERE doc_id CONTAINS $repo_id AND content CONTAINS $query LIMIT $limit";
        let results: Vec<VectorRecord> = self.client.query_with(sql, serde_json::json!({
            "repo_id": repo_id,
            "query": query,
            "limit": limit,
        })).await?;

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_chunk_text() {
        let indexer = Indexer::new(vec![], 0, 10, 2);
        let text = "abcdefghijklmnopqrstuvwxyz";
        let chunks = indexer.chunk_text(text);

        // chunk 0: abcdefghij (0-10)
        // chunk 1: ijklmnopqrs (8-18)
        // chunk 2: rstuvwxyz (16-26)

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "abcdefghij");
        assert_eq!(chunks[1].content, "ijklmnopqr");
        assert_eq!(chunks[2].content, "qrstuvwxyz");
    }

    #[test]
    fn test_scan() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        let hidden_dir = dir.path().join(".git");
        std::fs::create_dir(&hidden_dir).unwrap();
        let hidden_file = hidden_dir.join("config");
        File::create(&hidden_file).unwrap();

        let indexer = Indexer::default();
        let files = indexer.scan(dir.path());

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test.rs"));
    }
}
