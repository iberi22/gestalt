//! Tool handler implementations for the Gestalt MCP server.
//!
//! Each handler implements `mcp_protocol_sdk::core::tool::ToolHandler`
//! and wraps a Gestalt capability.

use async_trait::async_trait;
use mcp_protocol_sdk::core::tool::ToolHandler;
use mcp_protocol_sdk::protocol::types::{CallToolResult, ContentBlock};
use mcp_protocol_sdk::server::McpServer;
use mcp_protocol_sdk::McpResult;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Shared task store (simple in-memory)
// ---------------------------------------------------------------------------

static TASK_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Task {
    id: String,
    name: String,
    status: String,
    description: Option<String>,
    created_at: u64,
    result: Option<String>,
}

lazy_static::lazy_static! {
    static ref TASKS: Mutex<HashMap<String, Task>> = Mutex::new(HashMap::new());
}

fn current_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Helper to build CallToolResult responses
// ---------------------------------------------------------------------------

fn ok_result(text: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::text(text)],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    }
}

fn err_result(text: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::text(text)],
        is_error: Some(true),
        structured_content: None,
        meta: None,
    }
}

// ---------------------------------------------------------------------------
// Handler implementations
// ---------------------------------------------------------------------------

/// Echo back the input message
pub struct EchoHandler;

#[async_trait]
impl ToolHandler for EchoHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let message = arguments
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Hello from Gestalt MCP!")
            .to_string();
        Ok(ok_result(message))
    }
}

/// Analyze a project directory
pub struct AnalyzeProjectHandler;

#[async_trait]
impl ToolHandler for AnalyzeProjectHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        match analyze_project(path).await {
            Ok(analysis) => Ok(ok_result(
                serde_json::to_string_pretty(&analysis).unwrap_or_default(),
            )),
            Err(e) => Ok(err_result(format!("Error: {}", e))),
        }
    }
}

async fn analyze_project(path: &str) -> anyhow::Result<Value> {
    use ignore::WalkBuilder;
    use std::collections::BTreeMap;

    let mut total_files = 0u64;
    let mut total_dirs = 0u64;
    let mut total_size = 0u64;
    let mut languages: BTreeMap<String, u64> = BTreeMap::new();
    let mut main_files: Vec<String> = Vec::new();

    for entry in WalkBuilder::new(path)
        .standard_filters(true)
        .max_depth(Some(10))
        .build()
    {
        let entry = entry?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            total_dirs += 1;
            continue;
        }
        total_files += 1;

        if let Ok(meta) = entry.metadata() {
            total_size += meta.len();
        }

        // Detect language by extension
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            let lang = match ext {
                "rs" => "Rust",
                "ts" | "tsx" => "TypeScript",
                "js" | "jsx" => "JavaScript",
                "py" => "Python",
                "go" => "Go",
                "java" => "Java",
                "kt" | "kts" => "Kotlin",
                "swift" => "Swift",
                "c" | "h" => "C",
                "cpp" | "hpp" | "cc" | "cxx" => "C++",
                "rb" => "Ruby",
                "php" => "PHP",
                "cs" => "C#",
                "dart" => "Dart",
                "sh" | "bash" | "zsh" => "Shell",
                "toml" | "yaml" | "yml" | "json" | "md" => "Config/Docs",
                _ => "Other",
            }
            .to_string();
            *languages.entry(lang).or_insert(0) += 1;
        }

        // Track main source files
        let fname = entry.file_name().to_string_lossy().to_string();
        if fname.starts_with("main.")
            || fname.starts_with("lib.")
            || fname == "mod.rs"
            || fname == "index.ts"
            || fname == "index.js"
        {
            main_files.push(entry.path().to_string_lossy().to_string());
        }
    }

    Ok(serde_json::json!({
        "total_files": total_files,
        "total_dirs": total_dirs,
        "total_size_bytes": total_size,
        "total_size_kb": total_size / 1024,
        "languages": languages,
        "main_files": main_files,
        "path": path
    }))
}

/// Search code in a directory
pub struct SearchCodeHandler;

#[async_trait]
impl ToolHandler for SearchCodeHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let pattern = arguments
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let extensions = arguments
            .get("extensions")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if pattern.is_empty() {
            return Ok(err_result("Error: pattern is required".to_string()));
        }

        let results = search_code(pattern, path, extensions).await;
        let output = serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string());

        Ok(ok_result(output))
    }
}

async fn search_code(pattern: &str, path: &str, extensions: &str) -> Vec<Value> {
    let mut results = Vec::new();

    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--json")
        .arg("--line-number")
        .arg("--no-heading")
        .arg("-m")
        .arg("20")
        .arg(pattern)
        .arg(path);

    // Type filter via --type-add
    if !extensions.is_empty() {
        for ext in extensions.split(',') {
            let ext = ext.trim();
            if !ext.is_empty() {
                cmd.arg("--type-add")
                    .arg(format!("ext:*.{}", ext))
                    .arg("--type")
                    .arg("ext");
            }
        }
    }

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    match output {
        Ok(out) => {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Ok(parsed) = serde_json::from_str::<Value>(line) {
                    if parsed.get("type").and_then(|v| v.as_str()) == Some("match") {
                        if let Some(data) = parsed.get("data") {
                            results.push(serde_json::json!({
                                "file": data.get("path").and_then(|v| v.as_str()),
                                "line_number": data.get("line_number"),
                                "content": data.get("lines")
                                    .and_then(|l| l.get("text"))
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.trim()),
                            }));
                        }
                    }
                }
            }
        },
        Err(_) => {
            results.push(serde_json::json!({
                "error": "ripgrep not found. Install it with: sudo apt install ripgrep"
            }));
        },
    }

    results
}

/// Git status
pub struct GitStatusHandler;

#[async_trait]
impl ToolHandler for GitStatusHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let output = tokio::process::Command::new("git")
            .args(["-C", path, "status", "--short", "--branch"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{}\n--- stderr ---\n{}", stdout, stderr)
                };
                Ok(CallToolResult {
                    content: vec![ContentBlock::text(combined)],
                    is_error: Some(!out.status.success()),
                    structured_content: None,
                    meta: None,
                })
            },
            Err(e) => Ok(err_result(format!("git error: {}", e))),
        }
    }
}

/// Git log
pub struct GitLogHandler;

#[async_trait]
impl ToolHandler for GitLogHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let max_count = arguments
            .get("max_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(20);

        let output = tokio::process::Command::new("git")
            .args([
                "-C",
                path,
                "log",
                &format!("--max-count={}", max_count),
                "--oneline",
                "--decorate",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{}\n--- stderr ---\n{}", stdout, stderr)
                };
                Ok(CallToolResult {
                    content: vec![ContentBlock::text(combined)],
                    is_error: Some(!out.status.success()),
                    structured_content: None,
                    meta: None,
                })
            },
            Err(e) => Ok(err_result(format!("git error: {}", e))),
        }
    }
}

/// Read a file
pub struct FileReadHandler;

#[async_trait]
impl ToolHandler for FileReadHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let max_lines = arguments
            .get("lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        if path.is_empty() {
            return Ok(err_result("Error: path is required".to_string()));
        }

        let path = std::path::Path::new(path);
        if !path.exists() {
            return Ok(err_result(format!(
                "Error: file not found: {}",
                path.display()
            )));
        }

        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let truncated = if lines.len() > max_lines as usize {
                    let truncated_msg = format!(
                        "\n... (showing {} of {} lines. Use lines=N to see more)",
                        max_lines, total
                    );
                    let mut out: Vec<String> = lines[..max_lines as usize]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    out.push(truncated_msg);
                    out.join("\n")
                } else {
                    content
                };
                Ok(ok_result(truncated))
            },
            Err(e) => Ok(err_result(format!("Error reading file: {}", e))),
        }
    }
}

/// Directory tree
pub struct FileTreeHandler;

#[async_trait]
impl ToolHandler for FileTreeHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let max_depth = arguments.get("depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

        let mut tree_entries = Vec::new();
        build_tree(path, 0, max_depth, &mut tree_entries);

        let output =
            serde_json::to_string_pretty(&tree_entries).unwrap_or_else(|_| "[]".to_string());
        Ok(ok_result(output))
    }
}

fn build_tree(path: &str, current_depth: usize, max_depth: usize, entries: &mut Vec<Value>) {
    if current_depth > max_depth {
        return;
    }

    let dir = std::path::Path::new(path);
    if !dir.is_dir() {
        return;
    }

    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files and common noise directories
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }

            let ftype = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                "dir"
            } else {
                "file"
            };

            entries.push(serde_json::json!({
                "name": name,
                "path": entry.path().to_string_lossy(),
                "type": ftype,
                "depth": current_depth,
            }));

            if ftype == "dir" {
                build_tree(
                    &entry.path().to_string_lossy(),
                    current_depth + 1,
                    max_depth,
                    entries,
                );
            }
        }
    }
}

/// System information
pub struct SystemInfoHandler;

#[async_trait]
impl ToolHandler for SystemInfoHandler {
    async fn call(&self, _arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let info = serde_json::json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
            "cwd": cwd,
            "hostname": std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string()),
            "username": std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            "pid": std::process::id(),
        });

        Ok(ok_result(
            serde_json::to_string_pretty(&info).unwrap_or_default(),
        ))
    }
}

/// Execute a shell command
pub struct ShellExecuteHandler;

#[async_trait]
impl ToolHandler for ShellExecuteHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let command = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let timeout_secs = arguments
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        if command.is_empty() {
            return Ok(err_result("Error: command is required".to_string()));
        }

        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        match output {
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{}\n--- stderr ---\n{}", stdout, stderr)
                };
                Ok(CallToolResult {
                    content: vec![ContentBlock::text(combined)],
                    is_error: Some(!out.status.success()),
                    structured_content: None,
                    meta: None,
                })
            },
            Ok(Err(e)) => Ok(err_result(format!("Execution error: {}", e))),
            Err(_) => Ok(err_result(format!(
                "Timeout after {}s: {}",
                timeout_secs, command
            ))),
        }
    }
}

/// Create a task
pub struct TaskCreateHandler;

#[async_trait]
impl ToolHandler for TaskCreateHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let description = arguments.get("description").and_then(|v| v.as_str());

        if id.is_empty() || name.is_empty() {
            return Ok(err_result("Error: id and name are required".to_string()));
        }

        let task = Task {
            id: id.to_string(),
            name: name.to_string(),
            status: "pending".to_string(),
            description: description.map(|s| s.to_string()),
            created_at: current_epoch(),
            result: None,
        };

        {
            let mut store = TASKS.lock().unwrap();
            store.insert(id.to_string(), task);
        }

        TASK_COUNTER.fetch_add(1, Ordering::SeqCst);

        Ok(ok_result(
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "id": id,
                "name": name,
                "status": "pending"
            }))
            .unwrap_or_default(),
        ))
    }
}

/// List tasks
pub struct TaskListHandler;

#[async_trait]
impl ToolHandler for TaskListHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let status_filter = arguments.get("status").and_then(|v| v.as_str());

        let store = TASKS.lock().unwrap();
        let mut tasks: Vec<&Task> = store.values().collect();
        tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        if let Some(filter) = status_filter {
            tasks.retain(|t| t.status == filter);
        }

        let result: Vec<Value> = tasks
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "status": t.status,
                    "created_at": t.created_at,
                    "description": t.description,
                    "result": t.result
                })
            })
            .collect();

        Ok(ok_result(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        ))
    }
}

/// Get task status
pub struct TaskStatusHandler;

#[async_trait]
impl ToolHandler for TaskStatusHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<CallToolResult> {
        let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");

        if id.is_empty() {
            return Ok(err_result("Error: id is required".to_string()));
        }

        let store = TASKS.lock().unwrap();
        match store.get(id) {
            Some(task) => {
                let result = serde_json::json!({
                    "id": task.id,
                    "name": task.name,
                    "status": task.status,
                    "created_at": task.created_at,
                    "description": task.description,
                    "result": task.result
                });
                Ok(ok_result(
                    serde_json::to_string_pretty(&result).unwrap_or_default(),
                ))
            },
            None => Ok(err_result(format!("Task not found: {}", id))),
        }
    }
}

/// Register all standard (built-in) tools on an McpServer.
///
/// These are the tools from the original gestalt_mcp skeleton: echo, file
/// operations, git, shell, system info, task management, and code search.
pub async fn register_standard_tools(server: &McpServer) -> anyhow::Result<()> {
    // echo
    server
        .add_tool(
            "echo".to_string(),
            Some("Echo back input as-is".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Message to echo back"
                    }
                },
                "required": ["message"]
            }),
            EchoHandler,
        )
        .await?;

    // analyze_project
    server
        .add_tool(
            "analyze_project".to_string(),
            Some("Analyze a project directory: file count, languages, structure".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to project directory"
                    }
                },
                "required": ["path"]
            }),
            AnalyzeProjectHandler,
        )
        .await?;

    // search_code
    server
        .add_tool(
            "search_code".to_string(),
            Some("Search code in a project using ripgrep-style patterns".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Search pattern (regex)" },
                    "path": { "type": "string", "description": "Path to search in" },
                    "extensions": { "type": "string", "description": "File extensions filter (comma separated)" }
                },
                "required": ["pattern", "path"]
            }),
            SearchCodeHandler,
        )
        .await?;

    // git_status
    server
        .add_tool(
            "git_status".to_string(),
            Some("Show git status for a repository".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to git repository" }
                },
                "required": ["path"]
            }),
            GitStatusHandler,
        )
        .await?;

    // git_log
    server
        .add_tool(
            "git_log".to_string(),
            Some("Show git commit log".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to git repository" },
                    "max_count": { "type": "integer", "description": "Max commits to show" }
                },
                "required": ["path"]
            }),
            GitLogHandler,
        )
        .await?;

    // file_read
    server
        .add_tool(
            "file_read".to_string(),
            Some("Read file contents".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file" },
                    "lines": { "type": "integer", "description": "Max lines to read" }
                },
                "required": ["path"]
            }),
            FileReadHandler,
        )
        .await?;

    // file_tree
    server
        .add_tool(
            "file_tree".to_string(),
            Some("Show directory tree structure".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path" },
                    "depth": { "type": "integer", "description": "Max depth" }
                },
                "required": ["path"]
            }),
            FileTreeHandler,
        )
        .await?;

    // system_info
    server
        .add_tool(
            "system_info".to_string(),
            Some("Get system information (OS, arch, cwd, etc.)".to_string()),
            serde_json::json!({ "type": "object", "properties": {} }),
            SystemInfoHandler,
        )
        .await?;

    // shell_execute
    server
        .add_tool(
            "shell_execute".to_string(),
            Some("Execute a shell command and return output".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "timeout": { "type": "integer", "description": "Timeout in seconds" }
                },
                "required": ["command"]
            }),
            ShellExecuteHandler,
        )
        .await?;

    // task_create
    server
        .add_tool(
            "task_create".to_string(),
            Some("Create a new task".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Task ID" },
                    "name": { "type": "string", "description": "Task name" },
                    "description": { "type": "string", "description": "Task description" }
                },
                "required": ["id", "name"]
            }),
            TaskCreateHandler,
        )
        .await?;

    // task_list
    server
        .add_tool(
            "task_list".to_string(),
            Some("List tasks, optionally filtered by status".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "description": "Filter by status (pending, running, completed, failed)" },
                    "db": { "type": "string", "description": "Database file path" }
                }
            }),
            TaskListHandler,
        )
        .await?;

    // task_status
    server
        .add_tool(
            "task_status".to_string(),
            Some("Get task status by ID".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Task ID" },
                    "db": { "type": "string", "description": "Database file path" }
                },
                "required": ["id"]
            }),
            TaskStatusHandler,
        )
        .await?;

    Ok(())
}
