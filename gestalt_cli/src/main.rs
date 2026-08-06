//! OpenClaw ↔ Gestalt CLI
//!
//! CLI tool for interacting with Gestalt MCP Server and managing tasks.

mod agent_wrapper;
mod bus;
mod chain;
mod config;
mod observe;
mod repl;

use crate::config::CliConfig;
use crate::repl::{EchoHandler, InteractiveRepl};
use clap::{Parser, Subcommand};
use gestalt_core::ports::outbound::vfs::{OverlayFs, VirtualFS, VirtualFileSystem as VirtualFs};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use ulid::Ulid;

// gestalt-router types
use gestalt_core::application::agent::registry::AgentRegistry;
use gestalt_core::application::agent::xavier::XavierClient;
use gestalt_router::agent::SubprocessRunner;
use gestalt_router::router::Router;
use gestalt_router::run::{AgentSpec, RunSpec};
use gestalt_router::run_state::{AgentState, MemState, StateDb};

/// Simple task storage
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub result: Option<String>,
}

/// Output captured from execution of a CLI adapter
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// A standard interface to invoke external CLIs (such as agy, codex, claude, jules).
#[async_trait::async_trait]
pub trait CliAdapter: Send + Sync {
    /// Unique identifier for this adapter
    fn id(&self) -> &str;
    /// CLI command or path to executable
    fn command(&self) -> &str;
    /// CLI arguments
    fn args(&self) -> &[String];
    /// Environment variables for execution
    fn env(&self) -> &HashMap<String, String>;
    /// Paths allowed for read/write access
    fn allowed_paths(&self) -> &[PathBuf];
    /// Execution timeout
    fn timeout(&self) -> Duration;
    /// Asynchronously execute the external CLI and capture stdout/stderr and exit code
    async fn execute(&self) -> Result<AdapterOutput, String>;
}

/// Concrete implementation of the CliAdapter trait for external tools.
pub struct ExternalCliAdapter {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub allowed_paths: Vec<PathBuf>,
    pub timeout: Duration,
}

impl ExternalCliAdapter {
    pub fn new(
        id: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        allowed_paths: Vec<PathBuf>,
        timeout: Duration,
    ) -> Self {
        Self {
            id,
            command,
            args,
            env,
            allowed_paths,
            timeout,
        }
    }
}

#[async_trait::async_trait]
impl CliAdapter for ExternalCliAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn command(&self) -> &str {
        &self.command
    }

    fn args(&self) -> &[String] {
        &self.args
    }

    fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    fn allowed_paths(&self) -> &[PathBuf] {
        &self.allowed_paths
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    async fn execute(&self) -> Result<AdapterOutput, String> {
        let mut cmd = tokio::process::Command::new(&self.command);
        cmd.args(&self.args);
        cmd.envs(&self.env);

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to spawn {}: {}", self.command, e)),
        };

        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();

        let read_stdout = tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut stdout_bytes);
        let read_stderr = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_bytes);
        let wait_child = child.wait();

        let timeout_fut = tokio::time::sleep(self.timeout);

        tokio::select! {
            res = async {
                tokio::try_join!(read_stdout, read_stderr, wait_child)
            } => {
                match res {
                    Ok((_, _, exit_status)) => {
                        let stdout_str = String::from_utf8_lossy(&stdout_bytes).into_owned();
                        let stderr_str = String::from_utf8_lossy(&stderr_bytes).into_owned();
                        Ok(AdapterOutput {
                            stdout: stdout_str,
                            stderr: stderr_str,
                            exit_code: exit_status.code(),
                        })
                    }
                    Err(e) => Err(format!("Error reading process streams: {}", e)),
                }
            }
            _ = timeout_fut => {
                let _ = child.kill().await;
                Err("Timeout reached during execution".to_string())
            }
        }
    }
}

/// Registry to register and unregister external CLI tools (thread-safe).
pub struct AdapterRegistry {
    adapters: std::sync::Mutex<HashMap<String, Arc<dyn CliAdapter>>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, adapter: Arc<dyn CliAdapter>) {
        let mut lock = self.adapters.lock().unwrap();
        lock.insert(adapter.id().to_string(), adapter);
    }

    pub fn unregister(&self, id: &str) -> Option<Arc<dyn CliAdapter>> {
        let mut lock = self.adapters.lock().unwrap();
        lock.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn CliAdapter>> {
        let lock = self.adapters.lock().unwrap();
        lock.get(id).cloned()
    }

    pub fn list(&self) -> Vec<Arc<dyn CliAdapter>> {
        let lock = self.adapters.lock().unwrap();
        lock.values().cloned().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// CLI arguments
#[derive(Parser, Debug)]
#[command(name = "gestalt")]
#[command(version)]
#[command(about = "OpenClaw ↔ Gestalt Bridge CLI", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// MCP server URL (overrides config)
    #[arg(long, global = true)]
    url: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug, Clone)]
enum McpAction {
    /// Start the standalone MCP server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(long, default_value_t = 3000)]
        port: u16,

        /// Transport mode: "http" or "stdio"
        #[arg(long, default_value = "http")]
        transport: String,
    },
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Model Context Protocol (MCP) commands
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

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
    Status,

    /// Check environment sanity (doctor check)
    Doctor,

    /// List available tools
    Tools,

    /// Execute a tool
    Exec {
        /// Tool name
        #[arg()]
        tool: String,

        /// Arguments as JSON
        #[arg(short, long, default_value = "{}")]
        args: String,
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
        #[arg(long)]
        db: Option<String>,
    },

    /// List tasks
    TaskList {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Database file path
        #[arg(long)]
        db: Option<String>,
    },

    /// Get task status
    TaskStatus {
        /// Task ID
        #[arg()]
        id: String,

        /// Database file path
        #[arg(long)]
        db: Option<String>,
    },

    /// Analyze a project
    Analyze {
        /// Project path
        #[arg(default_value = ".")]
        path: String,
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
    },

    /// Git operations
    Git {
        /// Git command (status, log, branch)
        #[arg(default_value = "status")]
        subcommand: String,

        /// Repository path
        #[arg(default_value = ".")]
        path: String,
    },

    /// Read a file
    Read {
        /// File path
        #[arg()]
        path: String,

        /// Max lines
        #[arg(short, long, default_value_t = 100)]
        lines: usize,
    },

    /// Get file tree
    Tree {
        /// Directory path
        #[arg(default_value = ".")]
        path: String,

        /// Max depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },

    /// System info
    SysInfo,

    /// Start interactive REPL
    Repl,

    /// Run multiple tasks in parallel using OverlayFs isolation
    Swarm {
        /// Task description (can be specified multiple times)
        #[arg(long, value_name = "DESCRIPTION")]
        task: Vec<String>,

        /// Workspace directory for swarm operations
        #[arg(long, default_value = ".swarm")]
        workspace: String,
    },

    /// Run multi-agent orchestration via Gestalt Router
    Run {
        /// Task description for the orchestrated run
        #[arg(long)]
        task: String,

        /// Agent commands (can be specified multiple times, e.g. --agent "python agent.py")
        #[arg(long, value_name = "COMMAND")]
        agents: Vec<String>,

        /// Base git ref (branch or commit SHA)
        #[arg(long, default_value = "main")]
        base_ref: String,

        /// Maximum number of agents to run in parallel
        #[arg(long, default_value_t = 4)]
        max_parallel: usize,

        /// Timeout in seconds for each agent
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },

    /// Gestalt ↔ Xavier Cycle: memory search, index, and git-aware context
    Xavier {
        #[command(subcommand)]
        action: XavierAction,
    },

    /// Universal event bus: ingest + serve bus events (traceability layer)
    Bus {
        #[command(subcommand)]
        action: BusAction,
    },

    /// Xavier Thinking Loop: synthesize insights from recent executions
    Thinking {
        #[command(subcommand)]
        action: Option<ThinkingAction>,

        /// Force a run even if today's insight already exists
        #[arg(long)]
        force: bool,

        /// Look-back window in minutes
        #[arg(long, default_value_t = 30)]
        window: u64,

        /// Gated run: only run when ≥ MIN_EXECUTIONS new signal since last insight — never empty ticks
        #[arg(long)]
        gated: bool,
    },

    /// Observe active daemon for discovering agents, injecting hooks, and tracking artifacts
    Observe {
        /// Run discovery once and exit
        #[arg(long)]
        once: bool,
    },

    /// Run a chain of agents sequentially in dependency order
    Chain {
        #[command(subcommand)]
        action: ChainAction,
    },
}

#[derive(Subcommand, Debug)]
enum ChainAction {
    /// Run a chain specification
    Run {
        /// Path to the TOML specification file
        #[arg(long)]
        spec: String,

        /// Optional project name
        #[arg(long)]
        project: Option<String>,

        /// Continue executing subsequent steps even if a step fails
        #[arg(long)]
        continue_on_error: bool,
    },
}

#[derive(Subcommand, Debug)]
enum BusAction {
    /// Serve the event bus HTTP server (POST /api/event, GET /api/events)
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(long, default_value_t = 8081)]
        port: u16,

        /// StateDb path (default ~/.gestalt/state.db)
        #[arg(long)]
        db: Option<String>,
    },

    /// Push a single event to the bus (agent CLI integration)
    Push {
        /// Originating agent (hermes, jules, agent-cli, gestalt, ...)
        #[arg(long)]
        agent: String,

        /// Event type (run_started, run_finished, agent_state, decision, ...)
        #[arg(long)]
        event_type: String,

        /// One-line summary
        summary: String,

        /// Run id (optional)
        #[arg(long)]
        run_id: Option<String>,

        /// Project name (optional)
        #[arg(long)]
        project: Option<String>,

        /// Agent state (Pending|Running|Success|Timeout|Crashed)
        #[arg(long)]
        state: Option<String>,

        /// Extra traceability metadata as JSON ({"llm": "...", "provider": "...", "requested_by": "..."})
        #[arg(long)]
        metadata: Option<String>,
    },

    /// Replay unsynced bus events to Xavier (cursor sweep after outage)
    Replay {
        /// Only replay events newer than this sequence number
        #[arg(long)]
        after_seq: Option<i64>,

        /// Dry run: report what would be re-sunk without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Prune/retention of old event bus events (90 days window + archive Xavier)
    Prune {
        /// Cutoff in days (default 90)
        #[arg(long, default_value_t = 90)]
        days: u64,

        /// Archive pruned events to Xavier before deletion
        #[arg(long)]
        archive: bool,

        /// Dry run simulation (report count + oldest/newest affected, delete nothing)
        #[arg(long)]
        dry_run: bool,

        /// StateDb path (default ~/.gestalt/state.db)
        #[arg(long)]
        db: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ThinkingAction {
    /// List recent insights from Xavier
    List {
        /// Only list recent insights
        #[arg(long)]
        recent: bool,

        /// Max number of insights to retrieve
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Approve an insight and promote it to a decision
    Approve {
        /// The ID of the insight to approve
        #[arg(long)]
        id: String,

        /// Custom decision path under gestalt/decisions/<slug>
        #[arg(long)]
        path: Option<String>,

        /// Dry run: show what would be done without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum XavierAction {
    /// Search Xavier for context (PRE phase)
    Search {
        query: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
    },
    /// Index content in Xavier (POST phase)
    Add {
        content: String,
        #[arg(short, long)]
        path: String,
        #[arg(short, long, default_value = "execution")]
        kind: String,
    },
    /// Full PRE → EXEC → POST cycle with VFS isolation
    Cycle {
        task: String,
        #[arg(short, long)]
        agent: Option<String>,
        /// Base directory for VFS overlay (real files)
        #[arg(long)]
        vfs_dir: Option<String>,
        /// Overlay directory for isolated changes
        #[arg(long)]
        overlay_dir: Option<String>,
    },
    /// Show Xavier stats / health
    Stats,
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
    let content = serde_json::to_string_pretty(tasks).map_err(|e| e.to_string())?;
    fs::write(db_path, content).map_err(|e| e.to_string())
}

fn build_http_client() -> Result<reqwest::blocking::Client, String> {
    // Created inside spawn_blocking to avoid nesting tokio runtimes
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn call_mcp(
    client: &reqwest::blocking::Client,
    url: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
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
        .post(format!("{}/mcp", url))
        .json(&payload)
        .send()
        .map_err(|e| e.to_string())?;

    response.json().map_err(|e| e.to_string())
}

/// Execute a single swarm task using OverlayFs for file isolation.
async fn run_swarm_task(
    vfs: Arc<OverlayFs>,
    agent_id: &str,
    task_id: &str,
    task_desc: &str,
    workspace: &Path,
) -> Result<(), String> {
    // Write task manifest to isolated VFS
    let manifest_path = workspace.join("task_manifest.json");
    let manifest = serde_json::json!({
        "task_id": task_id,
        "agent_id": agent_id,
        "description": task_desc,
        "started_at": chrono::Utc::now().to_rfc3339(),
    });

    let manifest_str = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    vfs.write_string(&manifest_path, manifest_str, agent_id)
        .await
        .map_err(|e: anyhow::Error| e.to_string())?;

    // Write agent notes via OverlayFs
    let notes_path = workspace.join("notes.md");
    let notes = format!(
        "# Agent {} - Task {}\n\n## Description\n{}\n\n## Progress\n- Started: {}\n",
        agent_id,
        task_id,
        task_desc,
        chrono::Utc::now().to_rfc3339()
    );
    vfs.write_string(&notes_path, notes, agent_id)
        .await
        .map_err(|e: anyhow::Error| e.to_string())?;

    // Simulate work by executing via MCP tools if server is available
    let mcp_url = "http://127.0.0.1:3000";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    // Try to call analyze_project tool via MCP
    let args = json!({
        "path": workspace.to_string_lossy().to_string()
    });

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "analyze_project",
            "arguments": args
        },
        "id": 1
    });

    let response = client
        .post(format!("{}/mcp", mcp_url))
        .json(&payload)
        .send()
        .await;

    if let Ok(resp) = response {
        if resp.status().is_success() {
            let _analysis: Result<serde_json::Value, _> = resp.json().await;
            info!(
                "[{}] MCP tool executed successfully for agent {}",
                task_id, agent_id
            );
        }
    }

    // Update notes with completion
    let completion_notes = format!(
        "\n## Completed\n- Finished: {}\n- Status: SUCCESS\n",
        chrono::Utc::now().to_rfc3339()
    );
    let current_notes: String = vfs
        .read_to_string(&notes_path)
        .await
        .map_err(|e: anyhow::Error| e.to_string())?;
    let updated_notes = format!("{}{}", current_notes.trim_end(), completion_notes);
    vfs.write_string(&notes_path, updated_notes, agent_id)
        .await
        .map_err(|e: anyhow::Error| e.to_string())?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_registry_and_execution() {
        let registry = AdapterRegistry::new();

        let adapter = Arc::new(ExternalCliAdapter::new(
            "echo-adapter".to_string(),
            "echo".to_string(),
            vec!["hello world".to_string()],
            HashMap::new(),
            vec![],
            Duration::from_secs(5),
        ));

        registry.register(adapter.clone());

        let retrieved = registry.get("echo-adapter").unwrap();
        assert_eq!(retrieved.id(), "echo-adapter");
        assert_eq!(retrieved.command(), "echo");
        assert_eq!(retrieved.args(), &["hello world".to_string()]);

        let output = retrieved.execute().await.unwrap();
        assert!(output.stdout.contains("hello world"));
        assert_eq!(output.exit_code, Some(0));

        let unregistered = registry.unregister("echo-adapter").unwrap();
        assert_eq!(unregistered.id(), "echo-adapter");
        assert!(registry.get("echo-adapter").is_none());
    }

    #[tokio::test]
    async fn test_adapter_timeout() {
        // Sleep command will run for 10 seconds, but timeout is 1 second
        let adapter = ExternalCliAdapter::new(
            "sleep-adapter".to_string(),
            "sleep".to_string(),
            vec!["10".to_string()],
            HashMap::new(),
            vec![],
            Duration::from_millis(500),
        );

        let result = adapter.execute().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Timeout reached during execution");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CliConfig::load().unwrap_or_default();
    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose {
        "debug"
    } else {
        &config.logging.level
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    if config.logging.format == "json" {
        tracing_subscriber::registry()
            .with(fmt::layer().json())
            .with(filter)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(filter)
            .init();
    }

    let url = args.url.unwrap_or_else(|| config.mcp.server_url.clone());
    let default_db = "tasks.json";

    let xavier_url = std::env::var("XAVIER_URL").unwrap_or_else(|_| "http://127.0.0.1:8006".into());

    info!("Gestalt CLI starting with URL: {}", url);

    // Create blocking HTTP client inside spawn_blocking to avoid nested tokio runtime
    let http_client = tokio::task::spawn_blocking(build_http_client)
        .await
        .map_err(std::io::Error::other)??;

    match args.command {
        Commands::Mcp { action } => match action {
            McpAction::Serve { host, port, transport } => {
                info!("Starting standalone MCP Server on {}:{}", host, port);
                println!("🚀 Starting Standalone MCP Server on {}:{} ({})", host, port, transport);

                info!("Building gestalt_mcp...");
                println!("🔨 Building gestalt_mcp...");

                let build_status = tokio::process::Command::new("cargo")
                    .args(["build", "-p", "gestalt_mcp"])
                    .status()
                    .await?;

                if !build_status.success() {
                    error!("Failed to build gestalt_mcp");
                    std::process::exit(1);
                }

                info!("Starting MCP Server...");

                let mut child = tokio::process::Command::new("./target/debug/gestalt_mcp")
                    .arg("--transport")
                    .arg(transport)
                    .arg("--bind")
                    .arg(format!("{}:{}", host, port))
                    .spawn()?;

                let status = child.wait().await?;
                std::process::exit(status.code().unwrap_or(0));
            }
        },

        Commands::Serve { host, port } => {
            info!("Starting MCP Server on {}:{}", host, port);
            println!("🚀 Starting Gestalt MCP Server on {}:{}", host, port);
            println!("📍 URL: http://{}:{}", host, port);
            println!();

            // Build gestalt_mcp first, then run the binary directly (avoids blocking runtime panic)
            info!("Building gestalt_mcp...");
            println!("🔨 Building gestalt_mcp...");

            let build_status = tokio::process::Command::new("cargo")
                .args(["build", "-p", "gestalt_mcp"])
                .status()
                .await?;

            if !build_status.success() {
                error!("Failed to build gestalt_mcp");
                std::process::exit(1);
            }

            info!("Starting MCP Server...");

            let mut child = tokio::process::Command::new("./target/debug/gestalt_mcp")
                .arg("--http")
                .spawn()?;

            let status = child.wait().await?;
            std::process::exit(status.code().unwrap_or(0));
        },

        Commands::Status => {
            let tools_url = format!("{}/tools", url);
            let client = http_client.clone();
            let resp = tokio::task::spawn_blocking(move || client.get(&tools_url).send()).await??;

            match resp {
                resp if resp.status().is_success() => {
                    info!("MCP Server is online at {}", url);
                    println!("✅ Gestalt MCP Server: Online");
                    println!("📍 {}", url);
                },
                _ => {
                    warn!("MCP Server is offline at {}", url);
                    println!("❌ Gestalt MCP Server: Offline");
                    println!("📍 {}", url);
                    std::process::exit(1);
                },
            }
        },

        Commands::Doctor => {
            println!("🔍 Running Gestalt Doctor Environment Check...");
            println!("=============================================");
            let mut all_healthy = true;

            // 1. Xavier Reachability Check
            let xavier_client = XavierClient::from_env();
            match xavier_client.health().await {
                Ok(_) => {
                    println!(
                        "✅ Xavier reachability: Healthy (endpoint: {})",
                        xavier_client.endpoint
                    );
                },
                Err(e) => {
                    println!(
                        "❌ Xavier reachability: Unreachable (endpoint: {}): {}",
                        xavier_client.endpoint, e
                    );
                    all_healthy = false;
                },
            }

            // 2. StateDb Open Check
            let db_path = home::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".gestalt")
                .join("state.db");
            match StateDb::open(&db_path) {
                Ok(_) => {
                    println!("✅ StateDb open: Success (path: {})", db_path.display());
                },
                Err(e) => {
                    println!(
                        "❌ StateDb open: Failed (path: {}): {}",
                        db_path.display(),
                        e
                    );
                    all_healthy = false;
                },
            }

            // 3. Agent Registry Check
            let registry_path = std::path::Path::new("agent-registry.toml");
            match AgentRegistry::load(registry_path) {
                Ok(reg) => {
                    println!(
                        "✅ Agent registry parse: Success ({} agents loaded)",
                        reg.agents.len()
                    );
                },
                Err(e) => {
                    println!("❌ Agent registry parse: Failed to load: {}", e);
                    all_healthy = false;
                },
            }

            // 4. Bus Serve Reachability Check
            use std::net::TcpStream;
            match TcpStream::connect_timeout(
                &"127.0.0.1:8081".parse().unwrap(),
                Duration::from_secs(2),
            ) {
                Ok(_) => {
                    println!("✅ Bus serve reachability: Reachable (port 8081)");
                },
                Err(e) => {
                    println!("❌ Bus serve reachability: Unreachable (port 8081): {}", e);
                    all_healthy = false;
                },
            }

            println!("=============================================");
            if all_healthy {
                println!("Verdict: Healthy! All environment components are fully operational.");
                std::process::exit(0);
            } else {
                println!("Verdict: Unhealthy! Some environment checks failed. Please review the ❌ items above.");
                std::process::exit(1);
            }
        },

        Commands::Tools => {
            let tools_url = format!("{}/tools", url);
            let client = http_client.clone();
            let response =
                tokio::task::spawn_blocking(move || client.get(&tools_url).send()).await??;

            let tools: Vec<serde_json::Value> = response.json().map_err(|e| e.to_string())?;

            println!("📋 Available Tools ({}):", tools.len());
            for tool in tools {
                let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let desc = tool
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!("  • {}: {}", name, desc);
            }
        },

        Commands::Exec { tool, args } => {
            info!("Executing tool: {}", tool);
            let args_json: serde_json::Value = serde_json::from_str(&args).unwrap_or(json!({}));

            let url_clone = url.clone();
            let tool_clone = tool.clone();
            let client = http_client.clone();
            let result = tokio::task::spawn_blocking(move || {
                call_mcp(&client, &url_clone, &tool_clone, args_json)
            })
            .await?;

            match result {
                Ok(result) => {
                    info!("Tool {} executed successfully", tool);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).expect("Failed to serialize result")
                    );
                },
                Err(e) => {
                    error!("Failed to execute tool {}: {}", tool, e);
                    return Err(e.into());
                },
            }
        },

        Commands::TaskCreate {
            id,
            name,
            description,
            db,
        } => {
            let db_path = db.unwrap_or_else(|| default_db.to_string());
            info!("Creating task {} in database {}", id, db_path);
            let mut tasks = load_tasks(&db_path);

            let task = Task {
                id: id.clone(),
                name: name.clone(),
                status: "pending".to_string(),
                created_at: current_time(),
                result: description,
            };

            tasks.insert(id.clone(), task);
            save_tasks(&db_path, &tasks)?;

            info!("Task {} created successfully", id);
            println!("✅ Task created: {} ({})", name, id);
        },

        Commands::TaskList { status, db } => {
            let db_path = db.unwrap_or_else(|| default_db.to_string());
            let tasks = load_tasks(&db_path);

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
                println!(
                    "  {} [{}] {} - {}",
                    status_icon, task.status, task.id, task.name
                );
            }
        },

        Commands::TaskStatus { id, db } => {
            let db_path = db.unwrap_or_else(|| default_db.to_string());
            let tasks = load_tasks(&db_path);

            match tasks.get(&id) {
                Some(task) => {
                    println!("📝 Task: {} ({})", task.name, task.id);
                    println!("   Status: {}", task.status);
                    println!("   Created: {}", task.created_at);
                    if let Some(ref result) = task.result {
                        println!("   Result: {}", result);
                    }
                },
                None => {
                    println!("❌ Task not found: {}", id);
                    std::process::exit(1);
                },
            }
        },

        Commands::Analyze { path } => {
            let args = json!({ "path": path });
            let url_clone = url.clone();
            let client = http_client.clone();
            let result = tokio::task::spawn_blocking(move || {
                call_mcp(&client, &url_clone, "analyze_project", args)
            })
            .await??;

            if let Some(content) = result
                .get("result")
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
        },

        Commands::Search { pattern, path, ext } => {
            let args = json!({
                "pattern": pattern,
                "path": path,
                "extensions": ext
            });
            let url_clone = url.clone();
            let client = http_client.clone();
            let result = tokio::task::spawn_blocking(move || {
                call_mcp(&client, &url_clone, "search_code", args)
            })
            .await??;

            if let Some(content) = result
                .get("result")
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
                        println!(
                            "  {}:{} - {}",
                            file,
                            line,
                            &content[..content.len().min(60)]
                        );
                    }
                }
            }
        },

        Commands::Git { subcommand, path } => {
            let tool = match subcommand.as_str() {
                "status" => "git_status",
                "log" => "git_log",
                "branch" => "git_status",
                _ => "git_status",
            };

            let args = json!({ "path": path });
            let url_clone = url.clone();
            let client = http_client.clone();
            let result =
                tokio::task::spawn_blocking(move || call_mcp(&client, &url_clone, tool, args))
                    .await??;

            if let Some(content) = result
                .get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                println!("{}", content.as_str().unwrap_or(""));
            }
        },

        Commands::Read { path, lines } => {
            let args = json!({ "path": path, "lines": lines });
            let url_clone = url.clone();
            let client = http_client.clone();
            let result = tokio::task::spawn_blocking(move || {
                call_mcp(&client, &url_clone, "read_file", args)
            })
            .await??;

            if let Some(content) = result
                .get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
            {
                println!("{}", content.as_str().unwrap_or(""));
            }
        },

        Commands::Tree { path, depth } => {
            let args = json!({ "path": path, "depth": depth });
            let url_clone = url.clone();
            let client = http_client.clone();
            let result = tokio::task::spawn_blocking(move || {
                call_mcp(&client, &url_clone, "file_tree", args)
            })
            .await??;

            if let Some(content) = result
                .get("result")
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
        },

        Commands::SysInfo => {
            let args = json!({});
            let url_clone = url.clone();
            let client = http_client.clone();
            let result = tokio::task::spawn_blocking(move || {
                call_mcp(&client, &url_clone, "system_info", args)
            })
            .await??;

            if let Some(content) = result
                .get("result")
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
        },

        Commands::Repl => {
            info!("Starting interactive REPL");
            let mut repl = InteractiveRepl::with_handler(EchoHandler)?;
            repl.run().await?;
        },

        Commands::Swarm { task, workspace } => {
            if task.is_empty() {
                println!("❌ No tasks provided. Use --task \"description\" for each task.");
                std::process::exit(1);
            }

            info!(
                "Starting Swarm with {} task(s) in workspace '{}'",
                task.len(),
                workspace
            );
            println!("🐝 Swarm initiating with {} task(s)...", task.len());

            // Initialize OverlayFs for agent isolation
            let vfs = Arc::new(OverlayFs::new());
            let workspace_path = PathBuf::from(&workspace);

            // Create workspace directory
            if let Err(e) = std::fs::create_dir_all(&workspace_path) {
                error!("Failed to create workspace: {}", e);
                eprintln!("❌ Failed to create workspace '{}': {}", workspace, e);
                std::process::exit(1);
            }

            // Initialize timeline events
            let swarm_id = Ulid::new().to_string();
            let start_time = chrono::Utc::now();

            println!("📍 Swarm ID: {}", &swarm_id[..8]);
            println!("📁 Workspace: {}", workspace);
            println!();

            // Spawn all tasks
            let mut join_set = JoinSet::new();
            let vfs_clone = vfs.clone();

            for (idx, task_desc) in task.iter().enumerate() {
                let task_id = format!("{}-task-{}", &swarm_id[..8], idx);
                let vfs_for_task = vfs_clone.clone();
                let workspace_for_task = workspace_path.join(format!("agent_{}", idx));
                let task_desc = task_desc.clone();

                // Log task start (using tracing - TimelineService integration point)
                info!("[{}] Task START: {}", task_id, task_desc);
                println!("  🚀 [{}] Starting: {}", task_id, task_desc);

                join_set.spawn(async move {
                    let agent_id = format!("agent_{}", idx);

                    // Create isolated agent workspace
                    if let Err(e) = std::fs::create_dir_all(&workspace_for_task) {
                        error!("[{}] Failed to create agent dir: {}", task_id, e);
                        return (task_id, task_desc, Err(e.to_string()));
                    }

                    // Simulate agent work using gestalt_mcp tools via VFS
                    let result = run_swarm_task(
                        vfs_for_task.clone(),
                        &agent_id,
                        &task_id,
                        &task_desc,
                        &workspace_for_task,
                    )
                    .await;

                    // Log task completion
                    match &result {
                        Ok(_) => {
                            info!("[{}] Task COMPLETE: {}", task_id, task_desc);
                            println!("  ✅ [{}] Done: {}", task_id, task_desc);
                        },
                        Err(e) => {
                            info!("[{}] Task FAILED: {} - {}", task_id, task_desc, e);
                            println!("  ❌ [{}] Failed: {} ({})", task_id, task_desc, e);
                        },
                    }

                    (task_id, task_desc, result)
                });
            }

            // Wait for all tasks to complete
            let mut results = Vec::new();
            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok((task_id, task_desc, result)) => {
                        results.push((task_id, task_desc, result));
                    },
                    Err(e) => {
                        error!("Task panicked: {:?}", e);
                    },
                }
            }

            // Sort results by task_id for consistent output
            results.sort_by(|a, b| a.0.cmp(&b.0));

            println!();
            println!("📊 Swarm Results:");
            let successes = results.iter().filter(|r| r.2.is_ok()).count();
            let failures = results.len() - successes;
            for (task_id, task_desc, result) in &results {
                match result {
                    Ok(_) => println!("  ✅ {}: OK - {}", task_id, task_desc),
                    Err(e) => println!("  ❌ {}: FAIL - {} ({})", task_id, task_desc, e),
                }
            }

            println!();
            println!("💾 Flushing OverlayFs to disk...");

            // Flush OverlayFs to disk
            match vfs.flush().await {
                Ok(report) => {
                    if report.errors.is_empty() {
                        println!(
                            "  ✅ Flush complete: {} files, {} dirs written",
                            report.written_files.len(),
                            report.created_dirs.len()
                        );
                        for f in &report.written_files {
                            println!("     📄 {}", f.display());
                        }
                        for d in &report.created_dirs {
                            println!("     📁 {}", d.display());
                        }
                    } else {
                        println!("  ⚠️  Flush completed with {} errors:", report.errors.len());
                        for e in &report.errors {
                            println!(
                                "     ❌ {}: {} - {}",
                                e.path.display(),
                                e.operation,
                                e.error
                            );
                        }
                    }
                },
                Err(e) => {
                    error!("Flush failed: {}", e);
                    println!("  ❌ Flush failed: {}", e);
                },
            }

            let end_time = chrono::Utc::now();
            let duration = end_time.signed_duration_since(start_time);
            println!();
            println!(
                "🐝 Swarm complete in {}s ({} tasks: {} ✅ / {} ❌)",
                duration.num_seconds(),
                results.len(),
                successes,
                failures
            );

            // Exit with error if any tasks failed
            if failures > 0 {
                std::process::exit(1);
            }
        },

        Commands::Run {
            task,
            agents,
            base_ref,
            max_parallel,
            timeout,
        } => {
            info!(
                "Starting Gestalt Router run: task='{}', agents={}, max_parallel={}, timeout={}s",
                task,
                agents.len(),
                max_parallel,
                timeout
            );

            println!("🚀 Gestalt Router Run");
            println!("   Task: {}", task);
            println!("   Agents: {}", agents.len());
            println!("   Base ref: {}", base_ref);
            println!("   Max parallel: {}", max_parallel);
            println!("   Timeout: {}s", timeout);
            println!();

            // 1. Create SubprocessRunner with the configured timeout
            let timeout_dur = std::time::Duration::from_secs(timeout);
            let runner = SubprocessRunner::new(timeout_dur);

            // 4. Build AgentSpecs from the CLI agent commands
            let agent_specs: Vec<AgentSpec> = agents
                .iter()
                .enumerate()
                .map(|(idx, cmd)| {
                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                    let command = parts
                        .first()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| cmd.clone());
                    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                    AgentSpec {
                        id: format!("agent-{}", idx),
                        command,
                        args,
                        allowed_paths: None,
                        env: None,
                    }
                })
                .collect();

            // Save agent specs and count before moving into RunSpec
            let agent_specs_clone = agent_specs.clone();
            let agent_count = agent_specs.len();

            // 5. Build the RunSpec from CLI arguments
            let spec = RunSpec {
                base_ref: base_ref.clone(),
                task: task.clone(),
                agents: agent_specs.clone(),
                max_parallel,
                timeout,
                push: false,
                integration_branch: None,
            };

            // 6. Create Router with StateDb, MemState, XavierClient and execute
            let xavier_client = XavierClient::from_env();

            // Initialize local BM25 search engine for offline context retrieval
            let search_index_path = home::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".gestalt")
                .join("search_index");
            let search_engine =
                match gestalt_search::TantivySearchEngine::new(&search_index_path, 1) {
                    Ok(engine) => {
                        tracing::info!(
                            "Local BM25 search engine ready at {}",
                            search_index_path.display()
                        );
                        Some(Arc::new(engine)
                            as Arc<
                                dyn gestalt_core::ports::outbound::search::LocalSearchEngine,
                            >)
                    },
                    Err(e) => {
                        tracing::warn!("Failed to initialize local BM25 search: {}", e);
                        None
                    },
                };

            // Initialize state backends
            let state_db_path = home::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".gestalt")
                .join("state.db");
            let state_db =
                Arc::new(StateDb::open(&state_db_path).expect("Failed to open state database"));
            let mem_state = MemState::new();

            let mut router = Router::new(
                None, // VFS mode — Router creates WorktreeManager internally
                std::sync::Arc::new(runner),
                state_db,
                mem_state,
                None, // EventLog is now integrated via StateDbEventLog
                None, // WebSocket port — set Some(3001) to enable live events
            )
            .with_xavier(xavier_client);
            if let Some(engine) = search_engine {
                router = router.with_search_engine(engine);
            }
            println!("⚙️  Executing run...");

            match router.execute(spec).await {
                Ok(report) => {
                    println!();
                    println!("📊 Run Report:");
                    println!("   Run ID: {}", report.run_id);
                    println!("   Success: {}", report.success);
                    println!("   Agents: {}", report.agents.len());
                    println!("   Merged branches: {}", report.merged_branches.len());
                    println!("   Conflicts: {}", report.conflicts.len());
                    println!("   Events log: {}", report.events_path);
                    println!();

                    // Print individual agent results
                    for agent in &report.agents {
                        let icon = match agent.state {
                            AgentState::Success => "✅",
                            AgentState::NoChanges => "⏭️",
                            AgentState::Timeout => "⏰",
                            AgentState::Crashed => "💥",
                            AgentState::Quarantined => "⚠️",
                            AgentState::Pending => "⏳",
                            AgentState::Running => "🔄",
                        };
                        println!("   {} [{}] {:?}", icon, agent.agent_id, agent.state);
                        if let Some(ref err) = agent.error {
                            println!("      Error: {}", err);
                        }
                        if let Some(ref out) = agent.output {
                            if !out.is_empty() {
                                println!("      Output: {}...", &out[..out.len().min(200)]);
                            }
                        }
                        if !agent.changed_files.is_empty() {
                            println!("      Changed files: {}", agent.changed_files.len());
                        }
                        println!("      Duration: {}ms", agent.duration_ms);
                    }

                    // Print conflicts if any
                    if !report.conflicts.is_empty() {
                        println!();
                        println!("   ⚠️  Conflicts:");
                        for conflict in &report.conflicts {
                            println!("      - {}: {}", conflict.agent_id, conflict.path);
                        }
                    }

                    // ── AgentWrapper block-level editing integration ──
                    // After router execution, wrap each agent through AgentWrapper
                    // to capture diffs and send BlockEdit operations to a VirtualFS.
                    // The tracked paths are the worktree directories created by the router.
                    if agent_count > 0 {
                        let wrapper_vfs: std::sync::Arc<dyn VirtualFS> =
                            std::sync::Arc::new(agent_wrapper::InMemoryVfs::new());
                        info!(
                            "AgentWrapper: {} agents ready for block-level diff capture",
                            agent_count
                        );
                        println!();
                        println!("📦 AgentWrapper block-level editing:");
                        for agent_spec in &agent_specs_clone {
                            let _wrapper = agent_wrapper::AgentWrapper::new(
                                wrapper_vfs.clone(),
                                agent_spec.id.clone(),
                                report.run_id.to_string(),
                                agent_spec.command.clone(),
                            );
                            // In production, call wrapper.execute().await to run the
                            // agent through AgentWrapper and capture block-level diffs.
                            // For now we log the setup — the actual execution is handled
                            // by the Router above.
                            info!(
                                "AgentWrapper setup: id={}, command={}",
                                agent_spec.id, agent_spec.command,
                            );
                        }
                        println!("   ✅ {} AgentWrapper(s) configured", agent_count);
                    }

                    // Also output the full report as JSON for machine consumption
                    println!();
                    println!("{}", serde_json::to_string_pretty(&report)?);
                },
                Err(e) => {
                    error!("Router execution failed: {}", e);
                    eprintln!("❌ Router execution failed: {}", e);
                    std::process::exit(1);
                },
            }
        },

        Commands::Xavier { action } => {
            match action {
                XavierAction::Search { query, limit } => {
                    let client = reqwest::Client::new();
                    let resp = client
                        .post(format!("{}/v1/memories/search", xavier_url))
                        .json(&serde_json::json!({"query": query, "limit": limit}))
                        .send()
                        .await?;
                    let body: serde_json::Value = resp.json().await?;
                    let empty_results = vec![];
                    let results = body["results"].as_array().unwrap_or(&empty_results);
                    println!("Xavier Search ({} results):", results.len());
                    for (i, r) in results.iter().enumerate() {
                        let mem = r["memory"].as_str().unwrap_or("");
                        let kind = r["metadata"]["kind"].as_str().unwrap_or("?");
                        let first_line = mem.lines().next().unwrap_or("");
                        let preview = first_line.get(0..80.min(first_line.len())).unwrap_or("");
                        println!(" {}. [{}] {}", i + 1, kind, preview);
                    }
                },
                XavierAction::Add {
                    content,
                    path,
                    kind,
                } => {
                    let client = reqwest::Client::new();
                    let resp = client
                        .post(format!("{}/v1/memories", xavier_url))
                        .json(&serde_json::json!({"content": content, "path": path, "kind": kind}))
                        .send()
                        .await?;
                    let body: serde_json::Value = resp.json().await?;
                    println!("Archived: {}", body["id"].as_str().unwrap_or("ok"));
                },
                XavierAction::Stats => {
                    let client = reqwest::Client::new();
                    let resp = client
                        .post(format!("{}/v1/memories/search", xavier_url))
                        .json(&serde_json::json!({"query": "", "limit": 0}))
                        .send()
                        .await?;
                    let body: serde_json::Value = resp.json().await?;
                    let count = body["results"].as_array().map(|a| a.len()).unwrap_or(0);
                    println!("Xavier: {} memories found", count);
                },
                XavierAction::Cycle {
                    task,
                    agent,
                    vfs_dir,
                    overlay_dir,
                } => {
                    // ── VFS Isolation ──
                    let use_vfs_overlay = vfs_dir.is_some() && overlay_dir.is_some();
                    let _vfs = std::sync::Arc::new(agent_wrapper::InMemoryVfs::new());
                    info!("VFS isolation created for Cycle task: {}", task);

                    // ── VFS Overlay Preparation ──
                    if use_vfs_overlay {
                        let src = vfs_dir.as_ref().unwrap();
                        let dst = overlay_dir.as_ref().unwrap();
                        let src_path = Path::new(src);
                        let dst_path = Path::new(dst);

                        if !src_path.exists() {
                            eprintln!("❌ VFS directory does not exist: {}", src);
                            std::process::exit(1);
                        }

                        if dst_path.exists() {
                            std::fs::remove_dir_all(dst_path)
                                .map_err(|e| format!("Failed to remove existing overlay: {}", e))?;
                        }

                        info!("Copying VFS base '{}' to overlay '{}'", src, dst);
                        println!("📁 Copying '{}' → '{}'", src, dst);
                        let cp_status = std::process::Command::new("cp")
                            .args(["-a", src, dst])
                            .status()
                            .map_err(|e| format!("Failed to copy: {}", e))?;

                        if !cp_status.success() {
                            eprintln!("❌ Failed to copy VFS directory to overlay");
                            std::process::exit(1);
                        }
                        println!("✅ VFS overlay ready: {} → {}", src, dst);
                    }

                    // ── State: Pending ──
                    let mut state = AgentState::Pending;
                    println!("Cycle state: {:?}", state);

                    // ── PRE: Search Xavier ──
                    state = AgentState::Running;
                    println!("Cycle state: {:?} — PRE phase", state);

                    let client = reqwest::Client::new();
                    let resp = client
                        .post(format!("{}/v1/memories/search", xavier_url))
                        .json(&serde_json::json!({"query": &task, "limit": 3}))
                        .send()
                        .await?;
                    let search_body: serde_json::Value = resp.json().await?;
                    let empty_vec = vec![];
                    let search_results = search_body["results"].as_array().unwrap_or(&empty_vec);
                    let result_count = search_results.len();

                    // Build readable search context for archive
                    let mut search_context = String::new();
                    for (i, r) in search_results.iter().enumerate() {
                        let mem = r["memory"].as_str().unwrap_or("");
                        let kind = r["metadata"]["kind"].as_str().unwrap_or("?");
                        let preview = mem.lines().next().unwrap_or("");
                        search_context.push_str(&format!("  {}. [{}] {}\n", i + 1, kind, preview));
                    }
                    println!("PRE: {} results from Xavier", result_count);
                    if !search_context.is_empty() {
                        print!("Context:\n{}", search_context);
                    }

                    // ── GIT: Get context ──
                    let branch_output = std::process::Command::new("git")
                        .args(["rev-parse", "--abbrev-ref", "HEAD"])
                        .output();
                    let log_output = std::process::Command::new("git")
                        .args(["log", "--oneline", "-5"])
                        .output();

                    let mut git_context = String::new();
                    if let Ok(out) = &branch_output {
                        let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        println!("Branch: {}", branch);
                        git_context.push_str(&format!("Branch: {}\n", branch));
                    }
                    if let Ok(out) = &log_output {
                        let log = String::from_utf8_lossy(&out.stdout);
                        println!("Recent commits:\n{}", log);
                        git_context.push_str(&format!("Recent commits:\n{}", log));
                    }

                    // ── EXEC: Run agent with VFS + timeout ──
                    let mut agent_stdout = String::new();
                    let mut agent_stderr = String::new();
                    let mut agent_exit_status = String::new();
                    let mut agent_state = AgentState::Success;

                    if let Some(agent_cmd) = agent {
                        println!("Cycle state: {:?} — EXEC phase", state);

                        // Parse command into program + args (no sh -c)
                        let parts: Vec<&str> = agent_cmd.split_whitespace().collect();
                        let (program, args) = if parts.is_empty() {
                            (String::new(), vec![])
                        } else {
                            (
                                parts[0].to_string(),
                                parts[1..].iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                            )
                        };

                        // Run with 300s timeout
                        let timeout_dur = Duration::from_secs(300);
                        let agent_overlay = overlay_dir.clone();
                        let agent_future = tokio::task::spawn_blocking(move || {
                            let mut cmd = std::process::Command::new(&program);
                            cmd.args(&args);
                            if let Some(ref od) = agent_overlay {
                                cmd.current_dir(od);
                            }
                            cmd.output()
                        });

                        match tokio::time::timeout(timeout_dur, agent_future).await {
                            Ok(Ok(Ok(output))) => {
                                let out_stdout = String::from_utf8_lossy(&output.stdout);
                                let out_stderr = String::from_utf8_lossy(&output.stderr);

                                agent_stdout = out_stdout.to_string();
                                agent_stderr = out_stderr.to_string();
                                agent_exit_status = format!("{}", output.status);

                                println!("Agent exit: {}", output.status);
                                if output.status.success() {
                                    agent_state = AgentState::Success;
                                } else {
                                    agent_state = AgentState::Crashed;
                                }

                                if !out_stdout.is_empty() {
                                    println!("stdout: {}", out_stdout);
                                }
                                if !out_stderr.is_empty() {
                                    println!("stderr: {}", out_stderr);
                                }
                            },
                            Ok(Ok(Err(e))) => {
                                agent_state = AgentState::Crashed;
                                agent_stderr = format!("Agent subprocess error: {}", e);
                                agent_exit_status = "error".to_string();
                                eprintln!("Agent failed: {}", e);
                            },
                            Ok(Err(join_err)) => {
                                agent_state = AgentState::Crashed;
                                agent_stderr = format!("Agent task panicked: {}", join_err);
                                agent_exit_status = "panic".to_string();
                                eprintln!("Agent task panicked: {}", join_err);
                            },
                            Err(_elapsed) => {
                                agent_state = AgentState::Timeout;
                                agent_stderr = "Agent timed out after 300s".to_string();
                                agent_exit_status = "timeout".to_string();
                                eprintln!("Agent timed out after 300s");
                            },
                        }
                    }

                    // ── VFS Diff: Compare overlay vs base ──
                    let mut vfs_diff = String::new();
                    if use_vfs_overlay {
                        let src = vfs_dir.as_ref().unwrap();
                        let dst = overlay_dir.as_ref().unwrap();

                        match std::process::Command::new("diff")
                            .args(["-ruN", src, dst])
                            .output()
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    vfs_diff = "No changes detected between VFS base and overlay"
                                        .to_string();
                                } else {
                                    let diff_text =
                                        String::from_utf8_lossy(&output.stdout).to_string();
                                    let truncated = if diff_text.len() > 10000 {
                                        format!(
                                            "{}...\n[diff truncated at 10000 chars]",
                                            &diff_text[..10000]
                                        )
                                    } else {
                                        diff_text
                                    };
                                    vfs_diff = truncated;
                                    println!(
                                        "📊 VFS Diff:\n{}",
                                        &vfs_diff[..vfs_diff.len().min(2000)]
                                    );
                                }
                            },
                            Err(e) => {
                                vfs_diff = format!("(diff command unavailable: {})", e);
                                eprintln!("⚠️ Could not compute VFS diff: {}", e);
                            },
                        }
                    }

                    state = agent_state;

                    // ── POST: Archive full content ──
                    println!("Cycle state: {:?} — POST phase", state);

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    let archive_content = format!(
                        "Cycle: {}\n\n\
                         === PRE: Xavier Search ===\n\
                         Results found: {}\n\
                         {}\n\
                         === GIT Context ===\n\
                         {}\n\
                         === EXEC: Agent Output ===\n\
                         Exit: {}\n\
                         stdout:\n{}\n\
                         stderr:\n{}\n\
                         === VFS Diff ===\n\
                         {}\n\
                         === State ===\n\
                         {:?}\n",
                        task,
                        result_count,
                        search_context,
                        git_context,
                        agent_exit_status,
                        agent_stdout,
                        agent_stderr,
                        vfs_diff,
                        state,
                    );

                    let archive_body = serde_json::json!({
                        "content": archive_content,
                        "path": format!("gestalt/cycle/{}", timestamp),
                        "kind": "execution"
                    });

                    let archive_result = client
                        .post(format!("{}/v1/memories", xavier_url))
                        .json(&archive_body)
                        .send()
                        .await;

                    match archive_result {
                        Ok(resp) => {
                            let body: serde_json::Value = resp.json().await.unwrap_or_default();
                            println!(
                                "POST: archived (id={})",
                                body["id"].as_str().unwrap_or("ok")
                            );
                        },
                        Err(e) => {
                            eprintln!("POST archive failed: {}", e);
                        },
                    }

                    println!("Cycle complete — final state: {:?}", state);
                },
            }
        },

        Commands::Bus { action } => match action {
            BusAction::Serve { host, port, db } => {
                bus::serve(&host, port, db.as_deref()).await?;
            },
            BusAction::Push {
                agent,
                event_type,
                summary,
                run_id,
                project,
                state,
                metadata,
            } => {
                let mut ev = gestalt_router::event_bus::BusEvent::new(agent, event_type, summary);
                if let Some(run_id) = run_id {
                    ev = ev.with_run_id(run_id);
                }
                if let Some(project) = project {
                    ev = ev.with_project(project);
                }
                if let Some(state) = state {
                    ev = ev.with_state(state);
                }
                if let Some(metadata) = metadata {
                    let parsed: serde_json::Value = serde_json::from_str(&metadata)
                        .map_err(|e| format!("Invalid --metadata JSON: {}", e))?;
                    ev = ev.with_metadata(parsed);
                }

                let db_path = home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".gestalt")
                    .join("state.db");
                let db = Arc::new(
                    StateDb::open(&db_path)
                        .map_err(|e| format!("Failed to open StateDb: {}", e))?,
                );
                let sink = std::env::var("XAVIER_TOKEN")
                    .ok()
                    .filter(|t| !t.is_empty())
                    .map(|_| gestalt_router::xavier_sink::XavierEventSink::from_env());

                let seq = gestalt_router::event_bus::handle_event(&db, &ev, sink.as_ref())
                    .await
                    .map_err(|e| format!("Failed to push event: {}", e))?;
                match seq {
                    Some(seq) => println!(
                        "✅ Event pushed (seq={}) agent={} type={}",
                        seq, ev.agent, ev.event_type
                    ),
                    None => println!(
                        "⏭️  Event deduplicated (identical event within window) agent={} type={}",
                        ev.agent, ev.event_type
                    ),
                }
            },

            BusAction::Replay { after_seq, dry_run } => {
                use gestalt_router::event_bus::BusEvent;

                let db_path = home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".gestalt")
                    .join("state.db");
                let db = Arc::new(
                    StateDb::open(&db_path)
                        .map_err(|e| format!("Failed to open StateDb: {}", e))?,
                );

                let sink = std::env::var("XAVIER_TOKEN")
                    .ok()
                    .filter(|t| !t.is_empty())
                    .map(|_| gestalt_router::xavier_sink::XavierEventSink::from_env());

                let events = db
                    .recent_timeline(1000)
                    .map_err(|e| format!("Failed to read timeline: {}", e))?;

                let mut replayed = 0usize;
                let mut skipped = 0usize;
                for ev in events.iter().rev() {
                    if let Some(after) = after_seq {
                        if ev.seq.unwrap_or(0) <= after {
                            skipped += 1;
                            continue;
                        }
                    }
                    let parsed: BusEvent = match serde_json::from_str(&ev.payload) {
                        Ok(b) => b,
                        Err(_) => {
                            skipped += 1;
                            continue;
                        },
                    };
                    if dry_run {
                        println!(
                            "  [dry-run] seq={} {} {}",
                            ev.seq.unwrap_or(0),
                            parsed.agent,
                            parsed.event_type
                        );
                        replayed += 1;
                        continue;
                    }
                    if let Some(ref sink) = sink {
                        match sink.sink(&parsed).await {
                            Ok(()) => {
                                println!(
                                    "  ✅ seq={} {} {} → Xavier",
                                    ev.seq.unwrap_or(0),
                                    parsed.agent,
                                    parsed.event_type
                                );
                                replayed += 1;
                            },
                            Err(e) => {
                                eprintln!("  ❌ seq={} failed: {}", ev.seq.unwrap_or(0), e);
                            },
                        }
                    } else {
                        eprintln!("  ⚠️  XAVIER_TOKEN not set — cannot replay");
                        std::process::exit(1);
                    }
                }
                println!(
                    "Replay done: {} re-sunk, {} skipped (of {} total)",
                    replayed,
                    skipped,
                    events.len()
                );
            },

            BusAction::Prune {
                days,
                archive,
                dry_run,
                db,
            } => {
                let db_path = db.map(PathBuf::from).unwrap_or_else(|| {
                    home::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".gestalt")
                        .join("state.db")
                });

                let db = StateDb::open(&db_path)
                    .map_err(|e| format!("Failed to open StateDb: {}", e))?;

                let cutoff_ts = chrono::Utc::now() - chrono::Duration::days(days as i64);

                println!(
                    "Pruning events older than {} days (cutoff: {})...",
                    days, cutoff_ts
                );

                let count =
                    gestalt_router::event_bus::prune_events(&db, cutoff_ts, archive, dry_run)
                        .await
                        .map_err(|e| format!("Pruning failed: {}", e))?;

                if dry_run {
                    println!(
                        "[dry-run] Matched {} event(s). Delete/archive skipped.",
                        count
                    );
                } else {
                    println!("✅ Successfully pruned {} event(s).", count);
                }
            },
        },

        Commands::Thinking {
            action,
            force,
            window,
            gated,
        } => {
            use gestalt_router::thinking::ThinkingLoop;

            let xavier = Arc::new(XavierClient::from_env());

            if let Some(act) = action {
                match act {
                    ThinkingAction::List { recent: _, limit } => {
                        if !xavier.is_available().await {
                            eprintln!(
                                "❌ Xavier not reachable at :8006 — list subcommand needs Xavier"
                            );
                            std::process::exit(1);
                        }
                        println!("🔍 Listing recent insights from Xavier...");
                        let search_limit = (limit * 3).max(50);
                        match xavier
                            .search("gestalt/thinking/", search_limit, "hybrid")
                            .await
                        {
                            Ok(resp) => {
                                let mut filtered = Vec::new();
                                for r in resp.results {
                                    if r.path.starts_with("gestalt/thinking/") {
                                        filtered.push(r);
                                    }
                                }
                                filtered.truncate(limit);

                                println!("Found {} recent insights:", filtered.len());
                                for r in filtered {
                                    let date =
                                        r.path.strip_prefix("gestalt/thinking/").unwrap_or(&r.path);
                                    println!("ID: {}", r.id);
                                    println!("Date: {}", date);
                                    println!("Snippet: {}", r.text().trim());
                                    println!("----------------------------------------");
                                }
                            },
                            Err(e) => {
                                eprintln!("❌ Failed to search Xavier: {}", e);
                                std::process::exit(1);
                            },
                        }
                    },
                    ThinkingAction::Approve { id, path, dry_run } => {
                        if !xavier.is_available().await {
                            eprintln!("❌ Xavier not reachable at :8006 — approve subcommand needs Xavier");
                            std::process::exit(1);
                        }
                        println!("🔍 Searching for insight ID: {}...", id);
                        let mut found_memory = None;

                        if let Ok(resp) = xavier.search(&id, 10, "hybrid").await {
                            for r in resp.results {
                                if r.id == id {
                                    found_memory = Some(r);
                                    break;
                                }
                            }
                        }

                        if found_memory.is_none() {
                            if let Ok(resp) =
                                xavier.search("gestalt/thinking/", 100, "hybrid").await
                            {
                                for r in resp.results {
                                    if r.id == id {
                                        found_memory = Some(r);
                                        break;
                                    }
                                }
                            }
                        }

                        let r = match found_memory {
                            Some(m) => m,
                            None => {
                                eprintln!("❌ Insight memory with ID {} not found in Xavier.", id);
                                std::process::exit(1);
                            },
                        };

                        let content = r.text();
                        let slug = if let Some(ref p) = path {
                            if p.starts_with("gestalt/decisions/") {
                                p.clone()
                            } else {
                                format!("gestalt/decisions/{}", p)
                            }
                        } else {
                            let date_part =
                                r.path.strip_prefix("gestalt/thinking/").unwrap_or(&r.id);
                            let sanitized: String = date_part
                                .chars()
                                .map(|c| {
                                    if c.is_ascii_alphanumeric() {
                                        c.to_ascii_lowercase()
                                    } else {
                                        '-'
                                    }
                                })
                                .collect();
                            format!("gestalt/decisions/{}", sanitized)
                        };

                        let metadata = serde_json::json!({
                            "approved_by": "human",
                            "source_insight": id,
                        });

                        if dry_run {
                            println!("[dry-run] Would promote insight {} to decision at path {} with kind=decision", id, slug);
                            println!("[dry-run] Content: {}", content);
                            println!(
                                "[dry-run] Metadata: {}",
                                serde_json::to_string_pretty(&metadata).unwrap()
                            );
                        } else {
                            println!("🚀 Promoting insight to kind=decision at path {}...", slug);
                            match xavier.add(&content, &slug, "decision", metadata).await {
                                Ok(resp) => {
                                    println!("✅ Decision promoted successfully!");
                                    println!("   Memory ID: {}", resp.id);
                                    println!("   Status: {}", resp.status);
                                    println!("   Path: {}", slug);
                                },
                                Err(e) => {
                                    eprintln!("❌ Failed to promote decision in Xavier: {}", e);
                                    std::process::exit(1);
                                },
                            }
                        }
                    },
                }
            } else {
                if !xavier.is_available().await {
                    eprintln!("❌ Xavier not reachable at :8006 — thinking loop needs Xavier");
                    std::process::exit(1);
                }

                let synthesizer: Arc<dyn gestalt_router::thinking::InsightSynthesizer> =
                    Arc::new(bus::StructuralSynthesizer);
                let loop_ = ThinkingLoop::new(xavier, synthesizer).with_window(window);

                // Open the local StateDb — authoritative source of bus events.
                let db_path = home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".gestalt")
                    .join("state.db");
                let db = StateDb::open(&db_path)
                    .map_err(|e| format!("Failed to open StateDb: {}", e))?;

                println!("🧠 Gestalt Thinking Loop (window={}m)", window);

                if gated && !force {
                    println!(
                        "   Checking gated run policy (MIN_EXECUTIONS={})...",
                        gestalt_router::thinking::MIN_EXECUTIONS
                    );
                    if !loop_
                        .should_run(&db, gestalt_router::thinking::MIN_EXECUTIONS)
                        .await
                    {
                        let pending = loop_.pending_executions_since_last_insight(&db).await;
                        println!(
                            "   ℹ️  Gated run: only {} new executions (need ≥{}) since last insight. Refusing to run (never empty ticks).",
                            pending,
                            gestalt_router::thinking::MIN_EXECUTIONS
                        );
                        std::process::exit(0);
                    }
                }

                println!("   Pulling recent bus events from StateDb timeline...");

                let executions = loop_.recent_executions_from_db(&db, 100);
                println!("   {} recent bus executions found", executions.len());

                if executions.len() < gestalt_router::thinking::MIN_EXECUTIONS {
                    println!(
                        "   ℹ️  Only {} executions (need ≥{}) — not enough signal to think yet. Push more events with `gestalt bus push`.",
                        executions.len(),
                        gestalt_router::thinking::MIN_EXECUTIONS
                    );
                    std::process::exit(0);
                }

                if !force && loop_.has_today_insight().await? {
                    println!("   ℹ️  Today's insight already exists (use --force to re-run)");
                    std::process::exit(0);
                }

                println!("   Synthesizing deterministic insight (no LLM dependency)...");
                match loop_.run(&db, force).await? {
                    Some(insight) => {
                        println!("\n━━━ INSIGHT ━━━\n{}\n━━━━━━━━━━━━━", insight);
                        println!("✅ Insight indexed in Xavier as kind=insight");
                    },
                    None => println!("   No insight produced this cycle"),
                }
            }
        },

        Commands::Observe { once } => {
            if once {
                let results = observe::discover_agents()?;
                println!("[Discovery] Starting agent discovery pass...");
                println!(
                    "[Discovery] Detected {} agents in PATH:",
                    results.path_agents.len()
                );
                for agent in &results.path_agents {
                    println!("  - {}", agent.display());
                }
                println!(
                    "[Discovery] Detected {} config directories:",
                    results.config_dirs.len()
                );
                for dir in &results.config_dirs {
                    println!("  - {}", dir.display());
                }
                println!(
                    "[Discovery] Detected {} agents in Orca hooks:",
                    results.orca_hooks.len()
                );
                for hook in &results.orca_hooks {
                    println!("  - {}", hook.display());
                }
            } else {
                observe::run_daemon_loop().await?;
            }
        },

        Commands::Chain { action } => match action {
            ChainAction::Run {
                spec,
                project,
                continue_on_error,
            } => {
                info!("Starting chain run for spec: {}", spec);
                println!("⛓️  Running agent pipeline chain...");
                if let Err(e) = chain::run_chain(&spec, project, continue_on_error).await {
                    error!("Chain run failed: {}", e);
                    eprintln!("❌ Chain run failed: {}", e);
                    std::process::exit(1);
                }
                println!("✅ Chain run completed successfully.");
            },
        },
    }

    Ok(())
}
