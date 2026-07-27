use crate::ports::outbound::vfs::VfsError;
use thiserror::Error;

/// Comprehensive error enum for core domain and application operations within Gestalt.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Errors originating from the Versioned Virtual File System (VFS).
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),

    /// Errors originating from git or repository manager operations.
    #[error("Repository error: {0}")]
    Repository(String),

    /// Errors originating from Model Context Protocol (MCP) clients or registries.
    #[error("MCP error: {0}")]
    Mcp(String),

    /// Errors originating from Database / Persistence operations.
    #[error("Database error: {0}")]
    Database(String),

    /// Errors originating from Embedding models or RAG chunking / vector database operations.
    #[error("Embedding / RAG error: {0}")]
    Embedding(String),

    /// Errors originating from Agent execution or LLM interaction.
    #[error("Agent execution error: {0}")]
    Agent(String),

    /// Errors originating from workspace indexing.
    #[error("Indexing error: {0}")]
    Indexing(String),

    /// Errors originating from system or tool configuration.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Errors caused by validation failures (e.g. invalid branch names, unsafe paths, forbidden shell commands).
    #[error("Validation error: {0}")]
    Validation(String),

    /// Generic fallback for unexpected or unclassified failures.
    #[error("Internal/Other error: {0}")]
    Internal(String),
}

/// A specialized Result type for Gestalt Core operations.
pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_error_formatting() {
        let err_vfs = CoreError::Vfs(VfsError::NotFound("src/main.rs".to_string()));
        assert_eq!(err_vfs.to_string(), "VFS error: path not found: src/main.rs");

        let err_repo = CoreError::Repository("Git clone failed".to_string());
        assert_eq!(err_repo.to_string(), "Repository error: Git clone failed");

        let err_mcp = CoreError::Mcp("Tool not found".to_string());
        assert_eq!(err_mcp.to_string(), "MCP error: Tool not found");

        let err_db = CoreError::Database("Connection failed".to_string());
        assert_eq!(err_db.to_string(), "Database error: Connection failed");

        let err_embed = CoreError::Embedding("Dimension mismatch".to_string());
        assert_eq!(err_embed.to_string(), "Embedding / RAG error: Dimension mismatch");

        let err_agent = CoreError::Agent("Context timeout".to_string());
        assert_eq!(err_agent.to_string(), "Agent execution error: Context timeout");

        let err_idx = CoreError::Indexing("WalkDir failed".to_string());
        assert_eq!(err_idx.to_string(), "Indexing error: WalkDir failed");

        let err_cfg = CoreError::Config("Missing environment variable".to_string());
        assert_eq!(err_cfg.to_string(), "Configuration error: Missing environment variable");

        let err_val = CoreError::Validation("Forbidden metacharacter found".to_string());
        assert_eq!(err_val.to_string(), "Validation error: Forbidden metacharacter found");

        let err_int = CoreError::Internal("Unknown crash".to_string());
        assert_eq!(err_int.to_string(), "Internal/Other error: Unknown crash");
    }

    #[test]
    fn test_vfs_error_from_conversion() {
        let vfs_err = VfsError::LockContention("Already held".to_string());
        let core_err: CoreError = vfs_err.into();
        assert!(matches!(core_err, CoreError::Vfs(VfsError::LockContention(_))));
    }
}
