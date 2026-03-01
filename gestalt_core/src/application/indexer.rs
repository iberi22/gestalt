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
        let lines: Vec<&str> = text.lines().collect();
        let mut start_line = 0;

        while start_line < lines.len() {
            let mut chunk_content = String::new();
            let mut end_line = start_line;

            while end_line < lines.len() {
                let line = lines[end_line];
                if chunk_content.len() + line.len() + 1 > self.chunk_size && !chunk_content.is_empty() {
                    break;
                }
                chunk_content.push_str(line);
                chunk_content.push('\n');
                end_line += 1;
            }

            chunks.push(Chunk {
                content: chunk_content.trim_end().to_string(),
                index: chunks.len(),
            });

            if end_line == lines.len() {
                break;
            }

            // Move start_line forward but try to keep some overlap
            // We ensure progress by not going back further than start_line + 1
            let mut overlap_size = 0;
            let mut new_start = end_line;
            while new_start > start_line + 1 {
                let line_len = lines[new_start - 1].len() + 1;
                if overlap_size + line_len > self.chunk_overlap {
                    break;
                }
                overlap_size += line_len;
                new_start -= 1;
            }
            start_line = new_start;
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
        let text = "line1\nline2\nline3\nline4\nline5";
        let chunks = indexer.chunk_text(text);

        // chunk_size 10
        // chunk 0: line1\nline2 (11 chars, but wait, it should split at line2 because 5+1+5+1 > 10)
        // Actually my logic:
        // line1 (5 chars) + \n (1) = 6.
        // line2 (5) + 1 = 6. 6+6 = 12 > 10. So only line1.

        assert!(chunks.len() >= 5);
        assert_eq!(chunks[0].content, "line1");
        assert_eq!(chunks[1].content, "line2");
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
