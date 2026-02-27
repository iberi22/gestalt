//! Gestalt CLI - Standalone CLI for Gestalt MCP Server

use clap::{Parser, Subcommand};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Task structure
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
#[command(name = "gestaltctl")]
#[command(about = "Gestalt MCP CLI", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start MCP server
    Serve,
    
    /// Check server status
    Status,
    
    /// List tools
    Tools,
    
    /// Execute tool
    Exec {
        tool: String,
        #[arg(short, long, default_value = "{}")]
        args: String,
    },
    
    /// Create task
    TaskCreate {
        #[arg(short, long)]
        id: String,
        #[arg(short, long)]
        name: String,
    },
    
    /// List tasks
    TaskList,
    
    /// Task status
    TaskStatus {
        id: String,
    },
    
    /// Analyze project
    Analyze {
        #[arg(default_value = ".")]
        path: String,
    },
    
    /// Search code
    Search {
        pattern: String,
        #[arg(default_value = ".")]
        path: String,
    },
    
    /// Git status
    Git {
        #[arg(default_value = "status")]
        cmd: String,
        #[arg(default_value = ".")]
        path: String,
    },
    
    /// Read file
    Read {
        path: String,
        #[arg(short, long, default_value_t = 100)]
        lines: usize,
    },
    
    /// File tree
    Tree {
        #[arg(default_value = ".")]
        path: String,
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },
    
    /// System info
    SysInfo,
}

fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn load_tasks() -> HashMap<String, Task> {
    let path = PathBuf::from("tasks.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(tasks) = serde_json::from_str(&content) {
                return tasks;
            }
        }
    }
    HashMap::new()
}

fn save_tasks(tasks: &HashMap<String, Task>) -> Result<(), String> {
    let content = serde_json::to_string_pretty(tasks).map_err(|e| e.to_string())?;
    fs::write("tasks.json", content).map_err(|e| e.to_string())
}

fn call(tool: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": { "name": tool, "arguments": args },
        "id": 1
    });
    
    let resp = client
        .post("http://127.0.0.1:3000/mcp")
        .json(&payload)
        .send()
        .map_err(|e| e.to_string())?;
    
    resp.json().map_err(|e| e.to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    match args.command {
        Commands::Serve => {
            println!("🚀 Starting Gestalt MCP Server...");
            std::process::Command::new("cargo")
                .args(["run", "-p", "gestalt_mcp", "--", "--http"])
                .current_dir("E:/scripts-python/gestalt-rust")
                .status()?;
        }
        
        Commands::Status => {
            if reqwest::blocking::get("http://127.0.0.1:3000/tools").is_ok() {
                println!("✅ Server Online");
            } else {
                println!("❌ Server Offline");
                std::process::exit(1);
            }
        }
        
        Commands::Tools => {
            let tools: Vec<serde_json::Value> = reqwest::blocking::get("http://127.0.0.1:3000/tools")?
                .json()?;
            println!("📋 Tools ({}):", tools.len());
            for t in tools {
                println!("  • {}: {}", 
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                    t.get("description").and_then(|v| v.as_str()).unwrap_or("")
                );
            }
        }
        
        Commands::Exec { tool, args } => {
            let args_json: serde_json::Value = serde_json::from_str(&args).unwrap_or(json!({}));
            let result = call(&tool, args_json)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        
        Commands::TaskCreate { id, name } => {
            let mut tasks = load_tasks();
            tasks.insert(id.clone(), Task {
                id: id.clone(),
                name,
                status: "pending".to_string(),
                created_at: now(),
                result: None,
            });
            save_tasks(&tasks)?;
            println!("✅ Task created: {}", id);
        }
        
        Commands::TaskList => {
            let tasks = load_tasks();
            println!("📋 Tasks ({}):", tasks.len());
            for t in tasks.values() {
                let icon = match t.status.as_str() {
                    "completed" => "✅",
                    "running" => "🔄",
                    "failed" => "❌",
                    _ => "⏳",
                };
                println!("  {} [{}] {}", icon, t.status, t.id);
            }
        }
        
        Commands::TaskStatus { id } => {
            let tasks = load_tasks();
            match tasks.get(&id) {
                Some(t) => println!("[{}] {} - {}", t.status, t.id, t.name),
                None => { println!("❌ Not found"); std::process::exit(1); }
            }
        }
        
        Commands::Analyze { path } => {
            let result = call("analyze_project", json!({ "path": path }))?;
            if let Some(c) = result.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("text")) {
                if let Ok(a) = serde_json::from_str::<serde_json::Value>(c.as_str().unwrap_or("{}")) {
                    println!("📊 {} files", a.get("total_files").and_then(|v| v.as_u64()).unwrap_or(0));
                }
            }
        }
        
        Commands::Search { pattern, path } => {
            let result = call("search_code", json!({ "pattern": pattern, "path": path }))?;
            if let Some(c) = result.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("text")) {
                if let Ok(r) = serde_json::from_str::<Vec<serde_json::Value>>(c.as_str().unwrap_or("[]")) {
                    println!("🔍 {} results:", r.len());
                    for item in r.iter().take(10) {
                        println!("  {}:{} - {}", 
                            item.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                            item.get("line").and_then(|v| v.as_u64()).unwrap_or(0),
                            item.get("content").and_then(|v| v.as_str()).unwrap_or("").chars().take(50).collect::<String>()
                        );
                    }
                }
            }
        }
        
        Commands::Git { cmd, path } => {
            let tool = if cmd == "log" { "git_log" } else { "git_status" };
            let result = call(tool, json!({ "path": path }))?;
            if let Some(c) = result.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("text")) {
                println!("{}", c.as_str().unwrap_or(""));
            }
        }
        
        Commands::Read { path, lines } => {
            let result = call("read_file", json!({ "path": path, "lines": lines }))?;
            if let Some(c) = result.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("text")) {
                println!("{}", c.as_str().unwrap_or(""));
            }
        }
        
        Commands::Tree { path, depth } => {
            let result = call("file_tree", json!({ "path": path, "depth": depth }))?;
            if let Some(c) = result.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("text")) {
                if let Ok(tree) = serde_json::from_str::<Vec<serde_json::Value>>(c.as_str().unwrap_or("[]")) {
                    for t in tree.iter().take(30) {
                        let d = t.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
                        let n = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        println!("{}{}", "  ".repeat(d as usize), n);
                    }
                }
            }
        }
        
        Commands::SysInfo => {
            let result = call("system_info", json!({}))?;
            if let Some(c) = result.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("text")) {
                if let Ok(info) = serde_json::from_str::<serde_json::Value>(c.as_str().unwrap_or("{}")) {
                    println!("💻 System: {} {}", 
                        info.get("os").and_then(|v| v.as_str()).unwrap_or("?"),
                        info.get("arch").and_then(|v| v.as_str()).unwrap_or("")
                    );
                }
            }
        }
    }
    
    Ok(())
}
