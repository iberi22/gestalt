//! OpenClaw ↔ Gestalt CLI
//! 
//! CLI tool for interacting with Gestalt MCP Server and managing tasks.

use clap::{Parser, Subcommand};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Simple task storage
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub result: Option<String>,
}

/// CLI arguments
#[derive(Parser, Debug)]
#[command(name = "gestalt")]
#[command(about = "OpenClaw ↔ Gestalt Bridge CLI", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the MCP server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        
        /// Port to bind to
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
    
    /// Check server status
    Status {
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// List available tools
    Tools {
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// Execute a tool
    Exec {
        /// Tool name
        #[arg()]
        tool: String,
        
        /// Arguments as JSON
        #[arg(short, long, default_value = "{}")]
        args: String,
        
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// Create a task
    TaskCreate {
        /// Task ID
        #[arg(short, long)]
        id: String,
        
        /// Task name
        #[arg(short, long)]
        name: String,
        
        /// Task description
        #[arg(short, long)]
        description: Option<String>,
        
        /// Database file path
        #[arg(long, default_value = "tasks.json")]
        db: String,
    },
    
    /// List tasks
    TaskList {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
        
        /// Database file path
        #[arg(long, default_value = "tasks.json")]
        db: String,
    },
    
    /// Get task status
    TaskStatus {
        /// Task ID
        #[arg()]
        id: String,
        
        /// Database file path
        #[arg(long, default_value = "tasks.json")]
        db: String,
    },
    
    /// Analyze a project
    Analyze {
        /// Project path
        #[arg(default_value = ".")]
        path: String,
        
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// Search code
    Search {
        /// Search pattern
        #[arg()]
        pattern: String,
        
        /// Search path
        #[arg(default_value = ".")]
        path: String,
        
        /// File extensions
        #[arg(long, default_value = ".rs,.ts,.js,.py")]
        ext: String,
        
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// Git operations
    Git {
        /// Git command (status, log, branch)
        #[arg(default_value = "status")]
        subcommand: String,
        
        /// Repository path
        #[arg(default_value = ".")]
        path: String,
        
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// Read a file
    Read {
        /// File path
        #[arg()]
        path: String,
        
        /// Max lines
        #[arg(short, long, default_value_t = 100)]
        lines: usize,
        
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// Get file tree
    Tree {
        /// Directory path
        #[arg(default_value = ".")]
        path: String,
        
        /// Max depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
        
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
    
    /// System info
    SysInfo {
        /// MCP server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
    },
}

fn current_time() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", now)
}

fn load_tasks(db_path: &str) -> HashMap<String, Task> {
    let path = PathBuf::from(db_path);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(tasks) = serde_json::from_str(&content) {
                return tasks;
            }
        }
    }
    HashMap::new()
}

fn save_tasks(db_path: &str, tasks: &HashMap<String, Task>) -> Result<(), String> {
    let content = serde_json::to_string_pretty(tasks)
        .map_err(|e| e.to_string())?;
    fs::write(db_path, content).map_err(|e| e.to_string())
}

fn call_mcp(url: &str, tool: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();
    
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool,
            "arguments": args
        },
        "id": 1
    });
    
    let response = client
        .post(&format!("{}/mcp", url))
        .json(&payload)
        .send()
        .map_err(|e| e.to_string())?;
    
    response.json().map_err(|e| e.to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    match args.command {
        Commands::Serve { host, port } => {
            println!("🚀 Starting Gestalt MCP Server on {}:{}", host, port);
            println!("📍 URL: http://{}:{}", host, port);
            println!();
            
            // Run cargo
            let status = Command::new("cargo")
                .args(["run", "-p", "gestalt_mcp", "--", "--http"])
                .current_dir("E:/scripts-python/gestalt-rust")
                .status()?;
            
            std::process::exit(status.code().unwrap_or(0));
        }
        
        Commands::Status { url } => {
            let tools_url = format!("{}/tools", url);
            match reqwest::blocking::get(&tools_url) {
                Ok(resp) if resp.status().is_success() => {
                    println!("✅ Gestalt MCP Server: Online");
                    println!("📍 {}", url);
                }
                _ => {
                    println!("❌ Gestalt MCP Server: Offline");
                    println!("📍 {}", url);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::Tools { url } => {
            let tools_url = format!("{}/tools", url);
            let response = reqwest::blocking::get(&tools_url)
                .map_err(|e| e.to_string())?;
            let tools: Vec<serde_json::Value> = response.json()
                .map_err(|e| e.to_string())?;
            
            println!("📋 Available Tools ({}):", tools.len());
            for tool in tools {
                let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let desc = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
                println!("  • {}: {}", name, desc);
            }
        }
        
        Commands::Exec { tool, args, url } => {
            let args_json: serde_json::Value = serde_json::from_str(&args)
                .unwrap_or(json!({}));
            
            let result = call_mcp(&url, &tool, args_json)?;
            
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        
        Commands::TaskCreate { id, name, description, db } => {
            let mut tasks = load_tasks(&db);
            
            let task = Task {
                id: id.clone(),
                name: name.clone(),
                status: "pending".to_string(),
                created_at: current_time(),
                result: description,
            };
            
            tasks.insert(id.clone(), task);
            save_tasks(&db, &tasks)?;
            
            println!("✅ Task created: {} ({})", name, id);
        }
        
        Commands::TaskList { status, db } => {
            let tasks = load_tasks(&db);
            
            let mut task_list: Vec<&Task> = tasks.values().collect();
            task_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            
            if let Some(ref s) = status {
                task_list.retain(|t| t.status == *s);
            }
            
            println!("📋 Tasks ({}):", task_list.len());
            for task in task_list {
                let status_icon = match task.status.as_str() {
                    "completed" => "✅",
                    "running" => "🔄",
                    "failed" => "❌",
                    _ => "⏳",
                };
                println!("  {} [{}] {} - {}", status_icon, task.status, task.id, task.name);
            }
        }
        
        Commands::TaskStatus { id, db } => {
            let tasks = load_tasks(&db);
            
            match tasks.get(&id) {
                Some(task) => {
                    println!("📝 Task: {} ({})", task.name, task.id);
                    println!("   Status: {}", task.status);
                    println!("   Created: {}", task.created_at);
                    if let Some(ref result) = task.result {
                        println!("   Result: {}", result);
                    }
                }
                None => {
                    println!("❌ Task not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::Analyze { path, url } => {
            let args = json!({ "path": path });
            let result = call_mcp(&url, "analyze_project", args)?;
            
            if let Some(content) = result.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                let text = content.as_str().unwrap_or("");
                if let Ok(analysis) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Some(total) = analysis.get("total_files").and_then(|v| v.as_u64()) {
                        println!("📊 Project: {} files", total);
                    }
                    if let Some(files) = analysis.get("main_files").and_then(|v| v.as_array()) {
                        println!("   Main files: {}", files.len());
                    }
                }
            }
        }
        
        Commands::Search { pattern, path, ext, url } => {
            let args = json!({
                "pattern": pattern,
                "path": path,
                "extensions": ext
            });
            let result = call_mcp(&url, "search_code", args)?;
            
            if let Some(content) = result.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                let text = content.as_str().unwrap_or("[]");
                if let Ok(results) = serde_json::from_str::<Vec<serde_json::Value>>(text) {
                    println!("🔍 Found {} results:", results.len());
                    for r in results.iter().take(10) {
                        let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        let line = r.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                        let content = r.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        println!("  {}:{} - {}", file, line, &content[..content.len().min(60)]);
                    }
                }
            }
        }
        
        Commands::Git { subcommand, path, url } => {
            let tool = match subcommand.as_str() {
                "status" => "git_status",
                "log" => "git_log",
                "branch" => "git_status",
                _ => "git_status",
            };
            
            let args = json!({ "path": path });
            let result = call_mcp(&url, tool, args)?;
            
            if let Some(content) = result.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                println!("{}", content.as_str().unwrap_or(""));
            }
        }
        
        Commands::Read { path, lines, url } => {
            let args = json!({ "path": path, "lines": lines });
            let result = call_mcp(&url, "read_file", args)?;
            
            if let Some(content) = result.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                println!("{}", content.as_str().unwrap_or(""));
            }
        }
        
        Commands::Tree { path, depth, url } => {
            let args = json!({ "path": path, "depth": depth });
            let result = call_mcp(&url, "file_tree", args)?;
            
            if let Some(content) = result.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                let text = content.as_str().unwrap_or("[]");
                if let Ok(tree) = serde_json::from_str::<Vec<serde_json::Value>>(text) {
                    for t in tree.iter().take(30) {
                        let depth = t.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
                        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        println!("{}{}", "  ".repeat(depth as usize), name);
                    }
                }
            }
        }
        
        Commands::SysInfo { url } => {
            let args = json!({});
            let result = call_mcp(&url, "system_info", args)?;
            
            if let Some(content) = result.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                let text = content.as_str().unwrap_or("{}");
                if let Ok(info) = serde_json::from_str::<serde_json::Value>(text) {
                    println!("💻 System Info:");
                    if let Some(os) = info.get("os").and_then(|v| v.as_str()) {
                        println!("   OS: {}", os);
                    }
                    if let Some(arch) = info.get("arch").and_then(|v| v.as_str()) {
                        println!("   Arch: {}", arch);
                    }
                    if let Some(cwd) = info.get("cwd").and_then(|v| v.as_str()) {
                        println!("   CWD: {}", cwd);
                    }
                }
            }
        }
    }
    
    Ok(())
}
