mod bridge;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::PathBuf;
use std::process::Command;
use tokio::sync::Mutex;
use walkdir::WalkDir;

use bridge::BridgeState;

#[derive(Clone)]
pub struct AppState {
    pub workspace: PathBuf,
    pub bridge: BridgeState,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            bridge: BridgeState::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// ============ TOOLS ============

fn get_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "echo",
            "description": "Echoes back the input",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }
        }),
        serde_json::json!({
            "name": "analyze_project",
            "description": "Analyze project structure and return summary",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Project path (default: current)" }
                }
            }
        }),
        serde_json::json!({
            "name": "list_files",
            "description": "List files in directory with type info",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "depth": { "type": "integer", "description": "Max depth (default: 2)" },
                    "filter": { "type": "string", "description": "Filter by extension, e.g., '.rs,.toml'" }
                }
            }
        }),
        serde_json::json!({
            "name": "read_file",
            "description": "Read file contents",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "lines": { "type": "integer", "description": "Max lines to read" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "get_context",
            "description": "Get AI context about project (file tree, configs, README)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            }
        }),
        serde_json::json!({
            "name": "search_code",
            "description": "Search for pattern in code files",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" },
                    "extensions": { "type": "string", "description": "e.g., '.rs,.ts'" }
                },
                "required": ["pattern"]
            }
        }),
        serde_json::json!({
            "name": "exec_command",
            "description": "Execute shell command and return output",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeout": { "type": "integer", "description": "Seconds (default: 30)" },
                    "cwd": { "type": "string", "description": "Working directory" }
                },
                "required": ["command"]
            }
        }),
        serde_json::json!({
            "name": "git_status",
            "description": "Get git status of repository",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Repo path" }
                }
            }
        }),
        serde_json::json!({
            "name": "git_log",
            "description": "Get recent git commits",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "count": { "type": "integer", "description": "Number of commits (default: 5)" }
                }
            }
        }),
        serde_json::json!({
            "name": "file_tree",
            "description": "Get directory tree structure",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "depth": { "type": "integer", "description": "Max depth (default: 3)" },
                    "exclude": { "type": "string", "description": "Exclude patterns (comma-separated)" }
                }
            }
        }),
        serde_json::json!({
            "name": "grep",
            "description": "Grep-like search in files with context",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" },
                    "extensions": { "type": "string" },
                    "context": { "type": "integer", "description": "Lines of context (default: 2)" }
                },
                "required": ["pattern"]
            }
        }),
        serde_json::json!({
            "name": "create_file",
            "description": "Create or overwrite a file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }),
        serde_json::json!({
            "name": "web_fetch",
            "description": "Fetch URL content (simple)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "max_chars": { "type": "integer", "description": "Max characters (default: 5000)" }
                },
                "required": ["url"]
            }
        }),
        serde_json::json!({
            "name": "system_info",
            "description": "Get system information",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        serde_json::json!({
            "name": "task_create",
            "description": "Create a persistent task in the agentic bridge",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": { "type": "string", "description": "Unique task ID" },
                    "command": { "type": "string", "description": "Command to execute" }
                },
                "required": ["task_id", "command"]
            }
        }),
        serde_json::json!({
            "name": "task_status",
            "description": "Get status of a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" }
                },
                "required": ["task_id"]
            }
        }),
        serde_json::json!({
            "name": "task_list",
            "description": "List all active tasks",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        })
    ]
}

// ============ TOOL HANDLERS ============

fn handle_echo(args: &serde_json::Value) -> serde_json::Value {
    let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("hello");
    serde_json::json!({
        "content": [{ "type": "text", "text": format!("Echo: {}", message) }]
    })
}

fn handle_analyze_project(args: &serde_json::Value) -> serde_json::Value {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let mut lang_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut main_files: Vec<serde_json::Value> = Vec::new();
    let mut total: u32 = 0;
    
    for entry in WalkDir::new(&path).max_depth(3).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += 1;
            if let Some(ext) = entry.path().extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                *lang_counts.entry(ext_str.clone()).or_insert(0) += 1;
                
                let name = entry.file_name().to_string_lossy();
                if ["Cargo.toml", "package.json", "pyproject.toml", "main.rs", "lib.rs", "README.md", "go.mod", "requirements.txt"]
                    .iter()
                    .any(|&n| name == n)
                {
                    main_files.push(serde_json::json!({
                        "name": name,
                        "path": entry.path().to_string_lossy()
                    }));
                }
            }
        }
    }
    
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::json!({
                "path": path.to_string_lossy(),
                "languages": lang_counts,
                "total_files": total,
                "main_files": main_files
            })
        }]
    })
}

fn handle_list_files(args: &serde_json::Value) -> serde_json::Value {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    
    let filter = args.get("filter")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').map(|e| e.trim().to_lowercase()).collect::<Vec<_>>());
    
    let mut files: Vec<serde_json::Value> = Vec::new();
    
    for entry in WalkDir::new(&path).max_depth(depth).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path_str = entry.path().to_string_lossy().to_string();
            
            if let Some(ref filter_exts) = filter {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if !filter_exts.iter().any(|f| f == &ext_str || f == &format!(".{}", ext_str)) {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(serde_json::json!({
                "path": path_str,
                "size": size
            }));
        }
    }
    
    serde_json::json!({
        "content": [{ "type": "text", "text": files }]
    })
}

fn handle_read_file(args: &serde_json::Value) -> serde_json::Value {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => return error_response("Missing path parameter"),
    };
    
    let max_lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().take(max_lines).collect();
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": lines.join("\n")
                }]
            })
        }
        Err(e) => error_response(&format!("Failed to read file: {}", e))
    }
}

fn handle_get_context(args: &serde_json::Value) -> serde_json::Value {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let mut context = serde_json::json!({
        "project_type": "unknown",
        "structure": [],
        "configs": [],
        "readme": serde_json::Value::Null
    });
    
    if (path.join("Cargo.toml")).exists() {
        context["project_type"] = serde_json::json!("rust");
    } else if (path.join("package.json")).exists() {
        context["project_type"] = serde_json::json!("nodejs");
    } else if (path.join("pyproject.toml")).exists() {
        context["project_type"] = serde_json::json!("python");
    } else if (path.join("go.mod")).exists() {
        context["project_type"] = serde_json::json!("go");
    } else if (path.join("requirements.txt")).exists() {
        context["project_type"] = serde_json::json!("python");
    }
    
    let mut structure: Vec<String> = Vec::new();
    for entry in WalkDir::new(&path).max_depth(2).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_dir() && !entry.path().to_string_lossy().contains("node_modules") 
           && !entry.path().to_string_lossy().contains("target") && !entry.path().to_string_lossy().contains(".git") {
            let depth = entry.depth();
            let prefix = "  ".repeat(depth);
            structure.push(format!("{}{}", prefix, entry.file_name().to_string_lossy()));
        }
    }
    context["structure"] = serde_json::json!(structure);
    
    let configs = ["Cargo.toml", "package.json", "pyproject.toml", "gestalt.toml", "openclaw.json", "go.mod", "requirements.txt"];
    let found_configs: Vec<&str> = configs.iter()
        .filter(|c| path.join(c).exists())
        .map(|c| *c)
        .collect();
    context["configs"] = serde_json::json!(found_configs);
    
    let readme_paths = ["README.md", "readme.md", "README.txt"];
    for r in readme_paths {
        if let Ok(content) = std::fs::read_to_string(path.join(r)) {
            let preview: String = content.lines().take(20).collect::<Vec<_>>().join("\n");
            context["readme"] = serde_json::json!(preview);
            break;
        }
    }
    
    serde_json::json!({
        "content": [{ "type": "text", "text": context }]
    })
}

fn handle_search_code(args: &serde_json::Value) -> serde_json::Value {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return error_response("Missing pattern parameter"),
    };
    
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let extensions = args.get("extensions")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').map(|e| e.trim().to_lowercase()).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![".rs".to_string(), ".ts".to_string(), ".js".to_string(), ".py".to_string()]);
    
    let mut results: Vec<serde_json::Value> = Vec::new();
    
    for entry in WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let ext_match = entry.path().extension()
            .map(|ext| {
                let ext_str = format!(".{}", ext.to_string_lossy().to_lowercase());
                extensions.contains(&ext_str)
            })
            .unwrap_or(false);
        
        if !ext_match {
            continue;
        }
        
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for (line_num, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&pattern.to_lowercase()) {
                    results.push(serde_json::json!({
                        "file": entry.path().to_string_lossy(),
                        "line": line_num + 1,
                        "content": line.trim()
                    }));
                }
            }
        }
    }
    
    serde_json::json!({
        "content": [{ "type": "text", "text": results }]
    })
}

fn handle_exec_command(args: &serde_json::Value) -> serde_json::Value {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return error_response("Missing command parameter"),
    };
    
    let _timeout = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30) as u64;
    let cwd = args.get("cwd").and_then(|v| v.as_str()).map(PathBuf::from);
    
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd.unwrap_or_else(|| PathBuf::from(".")))
            .output()
    } else {
        Command::new("sh")
            .args(["-c", command])
            .current_dir(cwd.unwrap_or_else(|| PathBuf::from(".")))
            .output()
    };
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::json!({
                        "success": out.status.success(),
                        "exit_code": out.status.code(),
                        "stdout": stdout,
                        "stderr": stderr
                    })
                }]
            })
        }
        Err(e) => error_response(&format!("Failed to execute: {}", e))
    }
}

fn handle_git_status(args: &serde_json::Value) -> serde_json::Value {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&path)
        .output();
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": if stdout.is_empty() { "Clean".to_string() } else { stdout }
                }]
            })
        }
        Err(e) => error_response(&format!("Git error: {}", e))
    }
}

fn handle_git_log(args: &serde_json::Value) -> serde_json::Value {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
    
    let output = Command::new("git")
        .args(["log", &format!("--oneline -{}", count)])
        .current_dir(&path)
        .output();
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": stdout
                }]
            })
        }
        Err(e) => error_response(&format!("Git error: {}", e))
    }
}

fn handle_file_tree(args: &serde_json::Value) -> serde_json::Value {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    
    let exclude = args.get("exclude")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').collect::<Vec<_>>())
        .unwrap_or_else(|| vec!["node_modules", "target", ".git", "dist", "build"]);
    
    let mut tree: Vec<serde_json::Value> = Vec::new();
    
    for entry in WalkDir::new(&path).max_depth(depth).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path().to_string_lossy();
        
        if exclude.iter().any(|ex| entry_path.contains(ex)) {
            continue;
        }
        
        let depth = entry.depth();
        let prefix = "  ".repeat(depth);
        let icon = if entry.file_type().is_dir() { "📁" } else { "📄" };
        
        tree.push(serde_json::json!({
            "depth": depth,
            "name": format!("{} {}", icon, entry.file_name().to_string_lossy()),
            "path": entry_path
        }));
    }
    
    serde_json::json!({
        "content": [{ "type": "text", "text": tree }]
    })
}

fn handle_grep(args: &serde_json::Value) -> serde_json::Value {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return error_response("Missing pattern parameter"),
    };
    
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    
    let context = args.get("context").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    
    let extensions = args.get("extensions")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').map(|e| e.trim().to_lowercase()).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![".rs".to_string(), ".ts".to_string(), ".js".to_string(), ".py".to_string()]);
    
    let mut results: Vec<serde_json::Value> = Vec::new();
    
    for entry in WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let ext_match = entry.path().extension()
            .map(|ext| {
                let ext_str = format!(".{}", ext.to_string_lossy().to_lowercase());
                extensions.contains(&ext_str)
            })
            .unwrap_or(false);
        
        if !ext_match {
            continue;
        }
        
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            let lines: Vec<&str> = content.lines().collect();
            for (line_num, line) in lines.iter().enumerate() {
                if line.to_lowercase().contains(&pattern.to_lowercase()) {
                    let start = line_num.saturating_sub(context);
                    let end = (line_num + context + 1).min(lines.len());
                    let context_lines: Vec<String> = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(i, l)| format!("{:>4}: {}", start + i + 1, l))
                        .collect();
                    
                    results.push(serde_json::json!({
                        "file": entry.path().to_string_lossy(),
                        "line": line_num + 1,
                        "match": line.trim(),
                        "context": context_lines.join("\n")
                    }));
                }
            }
        }
    }
    
    serde_json::json!({
        "content": [{ "type": "text", "text": results }]
    })
}

fn handle_create_file(args: &serde_json::Value) -> serde_json::Value {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => return error_response("Missing path parameter"),
    };
    
    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return error_response("Missing content parameter"),
    };
    
    match std::fs::write(&path, content) {
        Ok(_) => serde_json::json!({
            "content": [{ "type": "text", "text": format!("File created: {}", path.to_string_lossy()) }]
        }),
        Err(e) => error_response(&format!("Failed to create file: {}", e))
    }
}

fn handle_web_fetch(args: &serde_json::Value) -> serde_json::Value {
    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return error_response("Missing url parameter"),
    };
    
    let max_chars = args.get("max_chars").and_then(|v| v.as_u64()).unwrap_or(5000) as usize;
    
    match reqwest::blocking::get(url) {
        Ok(resp) => {
            let text = resp.text().unwrap_or_default();
            let truncated = text.chars().take(max_chars).collect::<String>();
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": truncated
                }]
            })
        }
        Err(e) => error_response(&format!("Failed to fetch: {}", e))
    }
}

fn handle_system_info(args: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "cwd": std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
            })
        }]
    })
}

fn current_time() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", now)
}

fn handle_task_create(args: &serde_json::Value) -> serde_json::Value {
    let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("default");
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::json!({
                "task_id": task_id,
                "status": "created",
                "command": command,
                "created_at": current_time()
            })
        }]
    })
}

fn handle_task_status(args: &serde_json::Value) -> serde_json::Value {
    let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::json!({
                "task_id": task_id,
                "status": "pending"
            })
        }]
    })
}

fn handle_task_list(args: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::json!({
                "tasks": [],
                "count": 0
            })
        }]
    })
}

fn error_response(msg: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": msg }]
    })
}

fn handle_tool(name: &str, args: &serde_json::Value) -> serde_json::Value {
    match name {
        "echo" => handle_echo(args),
        "analyze_project" => handle_analyze_project(args),
        "list_files" => handle_list_files(args),
        "read_file" => handle_read_file(args),
        "get_context" => handle_get_context(args),
        "search_code" => handle_search_code(args),
        "exec_command" | "openclaw_exec" => handle_exec_command(args),
        "git_status" => handle_git_status(args),
        "git_log" => handle_git_log(args),
        "file_tree" => handle_file_tree(args),
        "grep" => handle_grep(args),
        "create_file" => handle_create_file(args),
        "web_fetch" => handle_web_fetch(args),
        "system_info" => handle_system_info(args),
        "task_create" => handle_task_create(args),
        "task_status" => handle_task_status(args),
        "task_list" => handle_task_list(args),
        _ => error_response(&format!("Tool not found: {}", name))
    }
}

// ============ SERVER ============

pub async fn start_server() -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(AppState::new()));

    let app = Router::new()
        .route("/mcp", post(handle_mcp_request))
        .route("/tools", get(handle_tools_list))
        .route("/sse", get(sse_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("🧠 Gestalt MCP Server v0.3.0 listening on {}", listener.local_addr()?);
    println!("📋 Available tools: {}", get_tools().len());
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_tools_list() -> Json<Vec<serde_json::Value>> {
    Json(get_tools())
}

async fn handle_mcp_request(
    State(_state): State<Arc<Mutex<AppState>>>,
    Json(payload): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    println!("📥 MCP Request: {}", payload.method);

    let result = match payload.method.as_str() {
        "initialize" => serde_json::json!({
            "protocolVersion": "0.1.0",
            "serverInfo": {
                "name": "gestalt-mcp",
                "version": "0.3.0"
            },
            "capabilities": {
                "tools": {},
                "context": true
            }
        }),
        "tools/list" => serde_json::json!({ "tools": get_tools() }),
        "tools/call" => {
            if let Some(params) = &payload.params {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
                handle_tool(name, &args)
            } else {
                return Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Missing params".to_string(),
                        data: None,
                    }),
                    id: payload.id,
                });
            }
        }
        _ => {
            return Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", payload.method),
                    data: None,
                }),
                id: payload.id,
            })
        }
    };

    Json(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: payload.id,
    })
}

pub async fn start_stdio_server() -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    eprintln!("🧠 Gestalt MCP Stdio Server Started");

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let req: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
        match req {
            Ok(req) => {
                let response = process_rpc_request(req).await;
                let resp_str = serde_json::to_string(&response)?;
                stdout.write_all(resp_str.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
            }
        }
    }
    Ok(())
}

async fn process_rpc_request(payload: JsonRpcRequest) -> JsonRpcResponse {
    let result = match payload.method.as_str() {
        "initialize" => serde_json::json!({
            "protocolVersion": "0.1.0",
            "serverInfo": { "name": "gestalt-mcp", "version": "0.3.0" },
            "capabilities": { "tools": {} }
        }),
        "tools/list" => serde_json::json!({ "tools": get_tools() }),
        "tools/call" => {
            if let Some(params) = &payload.params {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
                handle_tool(name, &args)
            } else {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError { code: -32602, message: "Missing params".to_string(), data: None }),
                    id: payload.id,
                };
            }
        }
        _ => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError { code: -32601, message: format!("Method not found: {}", payload.method), data: None }),
                id: payload.id,
            }
        }
    };

    JsonRpcResponse { jsonrpc: "2.0".to_string(), result: Some(result), error: None, id: payload.id }
}

async fn sse_handler() -> impl axum::response::IntoResponse {
    "SSE Not implemented yet"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_list() {
        let tools = get_tools();
        assert!(tools.len() > 10);
        assert!(tools.iter().any(|t| t.get("name").unwrap() == "echo"));
        assert!(tools.iter().any(|t| t.get("name").unwrap() == "analyze_project"));
    }
}
