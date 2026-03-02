//! CLI command definitions using Clap

use clap::{Parser, Subcommand};

/// Gestalt Timeline - Meta-Agent CLI Orchestrator
#[derive(Parser, Debug)]
#[command(name = "gestalt")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Output in JSON format for programmatic access
    #[arg(long, global = true)]
    pub json: bool,

    /// Enable Context Engine (default: true)
    #[arg(long, global = true, default_value_t = true)]
    pub context: bool,

    /// Agent mode: "build" (full access) or "plan" (read-only)
    #[arg(long, global = true, default_value = "build")]
    pub mode: String,

    /// The prompt to send to the agents (when not using subcommands)
    #[arg(short, long)]
    pub prompt: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Register a new project
    #[command(name = "add-project")]
    AddProject {
        /// Name of the project
        name: String,
    },

    /// Delete a project
    #[command(name = "delete-project")]
    DeleteProject {
        /// ID of the project to delete
        id: String,
    },

    /// Add a task to a project
    #[command(name = "add-task")]
    AddTask {
        /// Project name
        project: String,
        /// Task description
        description: String,
    },

    /// Update a task
    #[command(name = "update-task")]
    UpdateTask {
        /// Task ID
        id: String,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New status (Todo, InProgress, Completed, Cancelled)
        #[arg(long)]
        status: Option<String>,
    },

    /// Delete a task
    #[command(name = "delete-task")]
    DeleteTask {
        /// ID of the task to delete
        id: String,
    },

    /// Schedule a task for execution
    #[command(name = "schedule-task")]
    ScheduleTask {
        /// Task ID
        id: String,
        /// Execution time (ISO 8601 or duration like "1h")
        time: String,
    },

    /// Execute a task asynchronously
    #[command(name = "run-task")]
    RunTask {
        /// Task ID to execute
        task_id: String,
    },

    /// List all projects
    #[command(name = "list-projects")]
    ListProjects,

    /// List tasks, optionally filtered by project
    #[command(name = "list-tasks")]
    ListTasks {
        /// Optional project name to filter by
        project: Option<String>,
    },

    /// Show project status and progress
    #[command(name = "status")]
    Status {
        /// Project name
        project: String,
    },

    /// Show timeline of events
    #[command(name = "timeline")]
    Timeline {
        /// Filter events since duration (e.g., "1h", "30m", "1d")
        #[arg(long)]
        since: Option<String>,
    },

    /// Watch timeline events in real-time (persistent process)
    #[command(name = "watch")]
    Watch {
        /// Optional project to filter events
        #[arg(long)]
        project: Option<String>,

        /// Show only specific event types (comma-separated)
        #[arg(long)]
        events: Option<String>,
    },

    /// Broadcast a message to all connected agents
    #[command(name = "broadcast")]
    Broadcast {
        /// Message to broadcast
        message: String,

        /// Optional project context
        #[arg(long)]
        project: Option<String>,
    },

    /// Subscribe to events from a specific project
    #[command(name = "subscribe")]
    Subscribe {
        /// Project name to subscribe to
        project: String,
    },

    /// Register this agent with the system
    #[command(name = "agent-connect")]
    AgentConnect {
        /// Optional custom agent name
        #[arg(long)]
        name: Option<String>,
    },

    /// Disconnect this agent from the system
    #[command(name = "agent-disconnect")]
    AgentDisconnect,

    /// List all connected agents
    #[command(name = "list-agents")]
    ListAgents {
        /// Show only online agents
        #[arg(long)]
        online: bool,
    },

    /// Chat with AI orchestrator (Claude Sonnet 4.5)
    #[command(name = "ai-chat")]
    AiChat {
        /// Message to send to Claude
        message: String,
    },

    /// Execute workflow via AI orchestrator
    #[command(name = "ai-orchestrate")]
    AiOrchestrate {
        /// Natural language workflow description
        workflow: String,

        /// Optional project context
        #[arg(long)]
        project: Option<String>,

        /// Dry run - show actions without executing
        #[arg(long)]
        dry_run: bool,
    },

    /// Synchronize protocol states between SurrealDB and markdown files
    #[command(name = "protocol-sync")]
    ProtocolSync {
        /// Project name
        #[arg(short, long)]
        project: String,

        /// Path to the markdown file (default: .gitcore/planning/TASK.md)
        #[arg(short, long, default_value = ".gitcore/planning/TASK.md")]
        path: String,

        /// Synchronize from markdown to database
        #[arg(long)]
        to_db: bool,

        /// Synchronize from database to markdown
        #[arg(long)]
        to_markdown: bool,
    },

    /// Start the Agent REST API server
    #[command(name = "server")]
    Server {
        /// Port to listen on
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },

    /// Authenticate with Google Gemini via OAuth2
    #[command(name = "login")]
    Login,

    /// Start an interactive chat session
    #[command(name = "chat")]
    Chat,

    /// Index a repository for RAG-based code analysis
    #[command(name = "index-repo")]
    IndexRepo {
        /// Repository URL (GitHub, GitLab, or local path)
        url: String,
    },

    /// Start the Telegram Bot listener
    #[command(name = "bot")]
    Bot,

    /// 🚀 Start the Gestalt Nexus always-on agentic daemon.
    /// Runs TaskQueue + Telegram bot + AgentRuntime workers concurrently 24/7.
    #[command(name = "nexus")]
    Nexus {
        /// Max concurrent agent workers (default: 3)
        #[arg(long, default_value_t = 3)]
        workers: usize,
        /// Port for REST API (default: 3000)
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },

    /// Queue a task from the CLI for autonomous background execution
    #[command(name = "queue")]
    Queue {
        /// Natural language goal for an agent to work on
        goal: String,
        /// Priority 1-10 (default: 5)
        #[arg(long, default_value_t = 5)]
        priority: u8,
    },

    /// Manage sub-agent CLI processes
    #[command(name = "agent")]
    Agent {
        #[command(subcommand)]
        action: AgentCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// Launches a specific CLI in the background
    #[command(name = "spawn")]
    Spawn {
        /// Agent CLI command (e.g. codex, claude)
        agent_type: String,
        /// Initial prompt or command arguments
        prompt: String,
    },

    /// Lists currently running background agents managed by Gestalt
    #[command(name = "ps")]
    Ps,
}
