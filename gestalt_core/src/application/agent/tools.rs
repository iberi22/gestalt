use crate::context::scanner;
use crate::ports::outbound::repo_manager::{RepoManager, VectorDb};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use synapse_agentic::prelude::*;

pub async fn create_gestalt_tools(
    _repo_manager: Arc<dyn RepoManager>,
    vector_db: Arc<dyn VectorDb>,
) -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry.register_tool(ScanWorkspaceTool).await;
    registry.register_tool(SearchCodeTool { vector_db }).await;
    registry.register_tool(ExecuteShellTool).await;
    registry.register_tool(ReadFileTool).await;
    registry.register_tool(WriteFileTool).await;
    registry
}

pub struct ScanWorkspaceTool;

#[async_trait]
impl Tool for ScanWorkspaceTool {
    fn name(&self) -> &str {
        "scan_workspace"
    }
    fn description(&self) -> &str {
        "Generates a directory tree of the current project workspace."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "depth": { "type": "integer", "default": 2 }
            }
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
        let root = Path::new(".");
        let tree = scanner::generate_directory_tree(root, depth);
        Ok(json!({ "tree": tree }))
    }
}

pub struct SearchCodeTool {
    vector_db: Arc<dyn VectorDb>,
}

#[async_trait]
impl Tool for SearchCodeTool {
    fn name(&self) -> &str {
        "search_code"
    }
    fn description(&self) -> &str {
        "Search for similar code fragments in the vector database."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "default": 5 }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let _query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        // In a real implementation, we'd embed the query first.
        // For now, using a dummy zero vector just to verify trait linkage
        let similar = self
            .vector_db
            .search_similar("code", vec![0.0; 384], limit)
            .await?;

        Ok(json!({ "results": similar }))
    }
}

pub struct ExecuteShellTool;

#[async_trait]
impl Tool for ExecuteShellTool {
    fn name(&self) -> &str {
        "execute_shell"
    }
    fn description(&self) -> &str {
        "Execute a shell command on the host operating system."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The shell command to run" }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

        #[cfg(target_os = "windows")]
        let mut cmd = tokio::process::Command::new("powershell");
        #[cfg(target_os = "windows")]
        cmd.arg("-Command").arg(command);

        #[cfg(not(target_os = "windows"))]
        let mut cmd = tokio::process::Command::new("sh");
        #[cfg(not(target_os = "windows"))]
        cmd.arg("-c").arg(command);

        match cmd.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(json!({
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr
                }))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to execute '{}': {}", command, e)),
        }
    }
}

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read the contents of a file from the host filesystem."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The absolute or relative path to the file" }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(json!({ "content": content })),
            Err(e) => Err(anyhow::anyhow!("Failed to read file '{}': {}", path, e)),
        }
    }
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Write content to a file on the host filesystem."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The path to the file to create or overwrite" },
                "content": { "type": "string", "description": "The string content to write" }
            },
            "required": ["path", "content"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        match tokio::fs::write(path, content).await {
            Ok(_) => Ok(json!({ "success": true, "bytes_written": content.len() })),
            Err(e) => Err(anyhow::anyhow!("Failed to write file '{}': {}", path, e)),
        }
    }
}
