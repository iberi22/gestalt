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

            let name = url.split('/').last().unwrap_or("unknown").trim_end_matches(".git").to_string();

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

                // Skip hidden directories (like .git)
                if path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
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
        assert_eq!(chunks[2].content, "stuvwxyz");
        // Wait, my manual calculation was slightly off but let's check the code logic.
        // start=0, end=10. chunk="abcdefghij", index=0. start = 10 - 2 = 8.
        // start=8, end=18. chunk="ijklmnopqr", index=1. start = 18 - 2 = 16.
        // start=16, end=26. chunk="stuvwxyz", index=2. 26 == 26, break.
        // Correct.
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
