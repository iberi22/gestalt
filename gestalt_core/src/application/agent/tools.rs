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
    registry.register_tool(GitStatusTool).await;
    registry.register_tool(GitLogTool).await;
    registry.register_tool(GitBranchTool).await;
    registry.register_tool(GitAddTool).await;
    registry.register_tool(GitCommitTool).await;
    registry.register_tool(GitPushTool).await;

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

impl SearchCodeTool {
    pub fn new(vector_db: Arc<dyn VectorDb>, embedding_model: Arc<dyn EmbeddingModel>) -> Self {
        Self {
            vector_db,
            embedding_model,
        }
    }
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

        // Search in "chunks" collection which is used by IndexService
        let similar = self
            .vector_db
            .search_similar("chunks", query_embedding, limit)
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

fn validate_branch_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("branch name cannot be empty");
    }
    if name.contains("..")
        || name.starts_with('/')
        || name.ends_with('/')
        || name.starts_with('-')
        || name.contains('\\')
        || name.contains(' ')
    {
        anyhow::bail!("invalid branch name '{}'", name);
    }
    Ok(())
}

fn validate_git_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty() {
        anyhow::bail!("path cannot be empty");
    }
    if path.starts_with('/') || path.starts_with('\\') || path.contains("..") {
        anyhow::bail!("unsafe git path '{}'", path);
    }
    Ok(())
}

async fn run_git(args: &[String]) -> anyhow::Result<Value> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(json!({
        "exit_code": exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "success": exit_code == 0
    }))
}

pub struct GitStatusTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show git repository status in porcelain format."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, _args: Value) -> anyhow::Result<Value> {
        run_git(&[String::from("status"), String::from("--porcelain")]).await
    }
}

pub struct GitLogTool;

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }

    fn description(&self) -> &str {
        "Show recent git commits in one-line format."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer", "default": 5 }
            }
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(5);
        run_git(&[
            String::from("log"),
            String::from("--oneline"),
            format!("-{}", count),
        ])
        .await
    }
}

pub struct GitBranchTool;

#[async_trait]
impl Tool for GitBranchTool {
    fn name(&self) -> &str {
        "git_branch"
    }

    fn description(&self) -> &str {
        "List, create, or checkout git branches using safe arguments."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "create", "checkout"] },
                "name": { "type": "string", "description": "Branch name for create/checkout" },
                "checkout": { "type": "boolean", "default": false }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;

        match action {
            "list" => run_git(&[String::from("branch"), String::from("--list")]).await,
            "create" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' parameter"))?;
                validate_branch_name(name)?;
                let checkout = args
                    .get("checkout")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if checkout {
                    run_git(&[
                        String::from("checkout"),
                        String::from("-b"),
                        name.to_string(),
                    ])
                    .await
                } else {
                    run_git(&[String::from("branch"), name.to_string()]).await
                }
            }
            "checkout" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' parameter"))?;
                validate_branch_name(name)?;
                run_git(&[String::from("checkout"), name.to_string()]).await
            }
            _ => anyhow::bail!("unsupported git_branch action '{}'", action),
        }
    }
}

pub struct GitAddTool;

#[async_trait]
impl Tool for GitAddTool {
    fn name(&self) -> &str {
        "git_add"
    }

    fn description(&self) -> &str {
        "Stage one or more paths with git add, enforcing safe relative paths."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["paths"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let paths = args
            .get("paths")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing 'paths' parameter"))?;

        if paths.is_empty() {
            anyhow::bail!("paths cannot be empty");
        }

        let mut argv = vec![String::from("add")];
        for p in paths {
            let path = p
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("paths must contain strings"))?;
            validate_git_path(path)?;
            argv.push(path.to_string());
        }

        run_git(&argv).await
    }
}

pub struct GitCommitTool;

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Commit staged changes with a validated commit message."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'message' parameter"))?;

        if message.trim().is_empty() {
            anyhow::bail!("commit message cannot be empty");
        }

        run_git(&[
            String::from("commit"),
            String::from("-m"),
            message.to_string(),
        ])
        .await
    }
}

pub struct GitPushTool;

#[async_trait]
impl Tool for GitPushTool {
    fn name(&self) -> &str {
        "git_push"
    }

    fn description(&self) -> &str {
        "Push current branch to remote with safe branch validation."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "remote": { "type": "string", "default": "origin" },
                "branch": { "type": "string", "default": "main" }
            }
        })
    }

    async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
        let remote = args
            .get("remote")
            .and_then(|v| v.as_str())
            .unwrap_or("origin");
        let branch = args
            .get("branch")
            .and_then(|v| v.as_str())
            .unwrap_or("main");

        validate_branch_name(branch)?;

        run_git(&[String::from("push"), remote.to_string(), branch.to_string()]).await
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_branch_name, validate_git_path};

    #[test]
    fn branch_validation_rejects_unsafe_names() {
        assert!(validate_branch_name("feature/ok-name").is_ok());
        assert!(validate_branch_name("bad name").is_err());
        assert!(validate_branch_name("../bad").is_err());
        assert!(validate_branch_name("-bad").is_err());
    }

    #[test]
    fn git_path_validation_rejects_unsafe_paths() {
        assert!(validate_git_path("src/main.rs").is_ok());
        assert!(validate_git_path("../secret").is_err());
        assert!(validate_git_path("/etc/passwd").is_err());
        assert!(validate_git_path("\\\\server\\share").is_err());
    }
}
