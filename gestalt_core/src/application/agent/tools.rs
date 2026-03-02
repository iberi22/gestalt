use crate::context::scanner;
use crate::domain::rag::embeddings::{DummyEmbeddingModel, EmbeddingModel};
use crate::ports::outbound::repo_manager::{RepoManager, VectorDb};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use synapse_agentic::prelude::*;

pub async fn create_gestalt_tools(
    repo_manager: Arc<dyn RepoManager>,
    vector_db: Arc<dyn VectorDb>,
    llm_provider: Arc<dyn LLMProvider>,
) -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry.register_tool(ScanWorkspaceTool).await;

    // Use dummy embedding model for now as we don't have the model files easily accessible in all environments
    // In a real scenario, we'd initialize BertEmbeddingModel here.
    let embedding_model = Arc::new(DummyEmbeddingModel::new(384));

    registry
        .register_tool(SearchCodeTool {
            vector_db,
            embedding_model,
        })
        .await;
    registry.register_tool(ExecuteShellTool).await;
    registry.register_tool(ReadFileTool).await;
    registry.register_tool(WriteFileTool).await;

    registry
        .register_tool(CloneRepoTool {
            repo_manager: repo_manager.clone(),
        })
        .await;
    registry
        .register_tool(ListReposTool {
            repo_manager: repo_manager.clone(),
        })
        .await;
    registry.register_tool(AskAiTool { llm_provider }).await;

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
    embedding_model: Arc<dyn EmbeddingModel>,
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
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        let query_embedding = self.embedding_model.embed(query).await?;

        let similar = self
            .vector_db
            .search_similar("code", query_embedding, limit)
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

pub struct CloneRepoTool {
    pub repo_manager: Arc<dyn RepoManager>,
}

#[async_trait]
impl Tool for CloneRepoTool {
    fn name(&self) -> &str {
        "clone_repo"
    }
    fn description(&self) -> &str {
        "Clone a git repository to the local filesystem."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL of the git repository to clone" }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;

        let repo = self.repo_manager.clone_repo(url).await?;
        Ok(json!(repo))
    }
}

pub struct ListReposTool {
    pub repo_manager: Arc<dyn RepoManager>,
}

#[async_trait]
impl Tool for ListReposTool {
    fn name(&self) -> &str {
        "list_repos"
    }
    fn description(&self) -> &str {
        "List repositories accessible to the current user."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, _args: Value) -> anyhow::Result<Value> {
        let repos = self.repo_manager.list_repos().await?;
        Ok(json!(repos))
    }
}

pub struct AskAiTool {
    pub llm_provider: Arc<dyn LLMProvider>,
}

#[async_trait]
impl Tool for AskAiTool {
    fn name(&self) -> &str {
        "ask_ai"
    }
    fn description(&self) -> &str {
        "Ask a question to the AI model."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "The prompt to send to the AI" }
            },
            "required": ["prompt"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'prompt' parameter"))?;

        let response = self.llm_provider.generate(prompt).await?;
        Ok(json!({ "response": response }))
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
