//! MCP Tools Bridge - Exposes gestalt_mcp tools as ToolRegistry tools
//!
//! This module wraps the 18 MCP tools from gestalt_mcp crate and exposes them
//! as implementations of the synapse_agentic Tool trait, allowing them to be
//! registered in the gestalt_core ToolRegistry.
//!
//! The MCP server can still run independently on port 3000; this bridge simply
//! reuses the same handler logic for direct ToolRegistry calls.

use async_trait::async_trait;
use gestalt_mcp::{
    current_time, error_response, handle_analyze_project, handle_create_file,
    handle_echo, handle_exec_command, handle_file_tree, handle_get_context, handle_git_log,
    handle_git_status, handle_grep, handle_list_files, handle_read_file, handle_search_code,
    handle_system_info, handle_task_create, handle_task_list, handle_task_status,
    handle_web_fetch,
};
use serde_json::{json, Value};
use synapse_agentic::prelude::*;

// ============ Tool Implementations ============

macro_rules! sync_tool_wrapper {
    ($name:ident, $handler:ident, $description:expr, $parameters:expr) => {
        pub struct $name;

        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str {
                stringify!($name).strip_suffix("Tool").unwrap_or(stringify!($name))
            }
            fn description(&self) -> &str {
                $description
            }
            fn parameters(&self) -> Value {
                $parameters
            }

            async fn call(
                &self,
                _ctx: &dyn ToolContext,
                args: Value,
            ) -> anyhow::Result<Value> {
                // Run sync handler in blocking thread to avoid blocking async runtime
                let result = tokio::task::spawn_blocking(move || $handler(&args))
                    .await
                    .map_err(|e| anyhow::anyhow!("Task join error: {}", e))?;
                Ok(result)
            }
        }
    };
}

macro_rules! async_tool_wrapper {
    ($name:ident, $handler:ident, $description:expr, $parameters:expr) => {
        pub struct $name;

        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str {
                stringify!($name).strip_suffix("Tool").unwrap_or(stringify!($name))
            }
            fn description(&self) -> &str {
                $description
            }
            fn parameters(&self) -> Value {
                $parameters
            }

            async fn call(
                &self,
                _ctx: &dyn ToolContext,
                args: Value,
            ) -> anyhow::Result<Value> {
                let result = $handler(&args).await;
                Ok(result)
            }
        }
    };
}

// Sync tools (handlers that are fn, not async fn)
sync_tool_wrapper!(
    EchoTool,
    handle_echo,
    "Echoes back the input message.",
    json!({
        "type": "object",
        "properties": {
            "message": { "type": "string" }
        },
        "required": ["message"]
    })
);

sync_tool_wrapper!(
    AnalyzeProjectTool,
    handle_analyze_project,
    "Analyze project structure and return summary with language counts and main files.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Project path (default: current)" }
        }
    })
);

sync_tool_wrapper!(
    ListFilesTool,
    handle_list_files,
    "List files in directory with type info.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "depth": { "type": "integer", "description": "Max depth (default: 2)" },
            "filter": { "type": "string", "description": "Filter by extension, e.g., '.rs,.toml'" }
        }
    })
);

sync_tool_wrapper!(
    ReadFileToolMcp,
    handle_read_file,
    "Read file contents from the filesystem.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "lines": { "type": "integer", "description": "Max lines to read" }
        },
        "required": ["path"]
    })
);

sync_tool_wrapper!(
    GetContextTool,
    handle_get_context,
    "Get AI context about project (file tree, configs, README).",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" }
        }
    })
);

sync_tool_wrapper!(
    SearchCodeToolMcp,
    handle_search_code,
    "Search for pattern in code files.",
    json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string" },
            "path": { "type": "string" },
            "extensions": { "type": "string", "description": "e.g., '.rs,.ts'" }
        },
        "required": ["pattern"]
    })
);

sync_tool_wrapper!(
    FileTreeTool,
    handle_file_tree,
    "Get directory tree structure with depth control.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "depth": { "type": "integer", "description": "Max depth (default: 3)" },
            "exclude": { "type": "string", "description": "Exclude patterns (comma-separated)" }
        }
    })
);

sync_tool_wrapper!(
    GrepTool,
    handle_grep,
    "Grep-like search in files with context lines.",
    json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string" },
            "path": { "type": "string" },
            "extensions": { "type": "string" },
            "context": { "type": "integer", "description": "Lines of context (default: 2)" }
        },
        "required": ["pattern"]
    })
);

sync_tool_wrapper!(
    CreateFileTool,
    handle_create_file,
    "Create or overwrite a file.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "content": { "type": "string" }
        },
        "required": ["path", "content"]
    })
);

sync_tool_wrapper!(
    SystemInfoTool,
    handle_system_info,
    "Get system information (OS, architecture, current directory).",
    json!({
        "type": "object",
        "properties": {}
    })
);

sync_tool_wrapper!(
    TaskCreateTool,
    handle_task_create,
    "Create a persistent task in the agentic bridge.",
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "description": "Unique task ID" },
            "command": { "type": "string", "description": "Command to execute" }
        },
        "required": ["task_id", "command"]
    })
);

sync_tool_wrapper!(
    TaskStatusTool,
    handle_task_status,
    "Get status of a task.",
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string" }
        },
        "required": ["task_id"]
    })
);

sync_tool_wrapper!(
    TaskListTool,
    handle_task_list,
    "List all active tasks.",
    json!({
        "type": "object",
        "properties": {}
    })
);

// Async tools (handlers that are async fn)
async_tool_wrapper!(
    ExecCommandTool,
    handle_exec_command,
    "Execute shell command and return output.",
    json!({
        "type": "object",
        "properties": {
            "command": { "type": "string" },
            "timeout": { "type": "integer", "description": "Seconds (default: 30)" },
            "cwd": { "type": "string", "description": "Working directory" }
        },
        "required": ["command"]
    })
);

async_tool_wrapper!(
    GitStatusToolMcp,
    handle_git_status,
    "Get git status of repository in porcelain format.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Repo path" }
        }
    })
);

async_tool_wrapper!(
    GitLogToolMcp,
    handle_git_log,
    "Get recent git commits in one-line format.",
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "count": { "type": "integer", "description": "Number of commits (default: 5)" }
        }
    })
);

async_tool_wrapper!(
    WebFetchTool,
    handle_web_fetch,
    "Fetch URL content (simple HTTP GET).",
    json!({
        "type": "object",
        "properties": {
            "url": { "type": "string" },
            "max_chars": { "type": "integer", "description": "Max characters (default: 5000)" }
        },
        "required": ["url"]
    })
);

// ============ MCP Tools Bridge Builder ============

/// All MCP tools that can be registered in the ToolRegistry.
pub struct McpToolsBridge;

impl McpToolsBridge {
    /// Register all 18 MCP tools with the given registry.
    pub async fn register_all(registry: &ToolRegistry) {
        // Sync tools
        registry.register_tool(EchoTool).await;
        registry.register_tool(AnalyzeProjectTool).await;
        registry.register_tool(ListFilesTool).await;
        registry.register_tool(ReadFileToolMcp).await;
        registry.register_tool(GetContextTool).await;
        registry.register_tool(SearchCodeToolMcp).await;
        registry.register_tool(FileTreeTool).await;
        registry.register_tool(GrepTool).await;
        registry.register_tool(CreateFileTool).await;
        registry.register_tool(SystemInfoTool).await;
        registry.register_tool(TaskCreateTool).await;
        registry.register_tool(TaskStatusTool).await;
        registry.register_tool(TaskListTool).await;

        // Async tools
        registry.register_tool(ExecCommandTool).await;
        registry.register_tool(GitStatusToolMcp).await;
        registry.register_tool(GitLogToolMcp).await;
        registry.register_tool(WebFetchTool).await;
    }

    /// Returns the number of tools exposed by this bridge.
    pub const fn tool_count() -> usize {
        17
    }
}
