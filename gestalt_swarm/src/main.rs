use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, watch, RwLock, Semaphore};
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod health;
mod ingest;

use gestalt_core::application::agent::tools::{AskAiTool, ExecuteShellTool, GitStatusTool};
use health::{HealthChecker, HealthConfig, SwarmHealthMonitor};
use synapse_agentic::prelude::{
    GeminiProvider, GroqProvider, LLMProvider, MinimaxProvider, ToolRegistry,
};

// ============================================================================
// CLI
// ============================================================================

#[derive(Parser, Debug)]
#[command(name = "swarm")]
#[command(about = "🐝 Gestalt Swarm — Parallel Agent Execution", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Be quiet (less output)
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the swarm with parallel agents
    Run(RunArgs),
    /// Ingest execution metrics and run feedback loop
    Ingest {
        /// Run ID to associate metrics with
        #[arg(long)]
        run_id: String,

        /// JSON file with agent results (default: stdin)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
    /// Show current agent priorities from feedback loop
    Priorities {
        /// Filter by agent type
        #[arg(short, long)]
        agent_type: Option<String>,
    },
    /// Get recommended next steps from feedback loop
    NextSteps {
        /// Agent type to get steps for (default: all)
        #[arg(short, long)]
        agent_type: Option<String>,
    },
}

#[derive(Parser, Debug)]
#[command(name = "run")]
struct RunArgs {
    /// Number of parallel agents to spawn
    #[arg(short, long, default_value = "4")]
    agents: usize,

    /// Maximum concurrent LLM calls (bounded by API rate limits)
    #[arg(long, default_value = "8")]
    max_concurrency: usize,

    /// The goal/task for the swarm
    #[arg(short, long)]
    goal: String,

    /// Working directory for agents
    #[arg(short, long, value_hint = ValueHint::DirPath)]
    cwd: Option<PathBuf>,

    /// LLM provider to use
    #[arg(long, value_enum, default_value_t = LlmProviderKind::Gemini)]
    provider: LlmProviderKind,

    /// Model to use. Defaults depend on provider.
    #[arg(long)]
    model: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum LlmProviderKind {
    Gemini,
    Groq,
    Minimax,
}

// ============================================================================
// Agent Result
// ============================================================================

#[derive(Debug, Clone)]
struct AgentResult {
    agent_id: usize,
    success: bool,
    output: String,
    duration_ms: u64,
    tools_used: usize,
}

fn default_model(provider: LlmProviderKind) -> &'static str {
    match provider {
        LlmProviderKind::Gemini => "gemini-2.5-flash-lite",
        LlmProviderKind::Groq => "llama-3.3-70b-versatile",
        LlmProviderKind::Minimax => "MiniMax-Text-01",
    }
}

fn build_llm_provider(provider: LlmProviderKind, model: String) -> Result<Arc<dyn LLMProvider>> {
    match provider {
        LlmProviderKind::Gemini => {
            let api_key = std::env::var("GEMINI_API_KEY")
                .map_err(|_| anyhow::anyhow!("GEMINI_API_KEY is required for --provider gemini"))?;
            Ok(Arc::new(GeminiProvider::new(api_key, model)))
        }
        LlmProviderKind::Groq => {
            let api_key = std::env::var("GROQ_API_KEY")
                .map_err(|_| anyhow::anyhow!("GROQ_API_KEY is required for --provider groq"))?;
            Ok(Arc::new(GroqProvider::new(api_key, model)))
        }
        LlmProviderKind::Minimax => {
            let api_key = std::env::var("MINIMAX_API_KEY").map_err(|_| {
                anyhow::anyhow!("MINIMAX_API_KEY is required for --provider minimax")
            })?;
            let group_id = std::env::var("MINIMAX_GROUP_ID").map_err(|_| {
                anyhow::anyhow!("MINIMAX_GROUP_ID is required for --provider minimax")
            })?;
            Ok(Arc::new(MinimaxProvider::new(api_key, group_id, model)))
        }
    }
}

// ============================================================================
// Swarm Execution
// ============================================================================

async fn run_agent(
    agent_id: usize,
    goal: String,
    cwd: PathBuf,
    provider: LlmProviderKind,
    model: String,
    semaphore: Arc<Semaphore>,
    results: Arc<RwLock<Vec<AgentResult>>>,
    quiet: bool,
    monitor: Arc<SwarmHealthMonitor>,
) {
    let start = Instant::now();
    let permit = match semaphore.acquire().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to acquire permit: {}", e);
            return;
        }
    };

    if !quiet {
        println!("🟢 Agent {} started (cwd: {:?})", agent_id, cwd);
    }

    // Register with health monitor
    monitor.register_agent(agent_id).await;

    // Start heartbeat task
    let monitor_clone = monitor.clone();
    let heartbeat_agent_id = agent_id;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(5000));
        loop {
            interval.tick().await;
            monitor_clone.heartbeat(heartbeat_agent_id).await;
        }
    });

    let mut tools_used = 0;
    let success;
    let output;

    let llm = match build_llm_provider(provider, model.clone()) {
        Ok(provider) => provider,
        Err(e) => {
            success = false;
            output = format!("Agent {} failed before LLM call: {}", agent_id, e);
            monitor.report_error(agent_id, e.to_string()).await;
            let duration_ms = start.elapsed().as_millis() as u64;
            let mut r = results.write().await;
            r.push(AgentResult {
                agent_id,
                success,
                output,
                duration_ms,
                tools_used,
            });
            monitor.report_task_complete(agent_id, false).await;
            monitor.unregister_agent(agent_id).await;
            drop(permit);
            return;
        }
    };

    // Build minimal tool registry for this agent
    let registry = ToolRegistry::new();
    registry.register_tool(ExecuteShellTool).await;
    registry.register_tool(GitStatusTool).await;
    registry
        .register_tool(AskAiTool {
            llm_provider: llm.clone(),
        })
        .await;

    // Execute the goal
    let prompt = format!(
        "[Agent {}] Task: {}\n\
        Working directory: {:?}\n\
        Provider: {:?}\n\
        Model: {}\n\
        Execute this task and report results concisely.\n\
        Use tools: execute_shell, git_status, ask_ai\n",
        agent_id, goal, cwd, provider, model
    );

    match llm.generate(&prompt).await {
        Ok(response) => {
            success = true;
            output = response;
            tools_used = 1;
            monitor.report_task_complete(agent_id, true).await;
            if !quiet {
                println!("✅ Agent {} completed successfully", agent_id);
            }
        }
        Err(e) => {
            success = false;
            output = format!("Agent {} failed: {}", agent_id, e);
            monitor.report_error(agent_id, e.to_string()).await;
            monitor.report_task_complete(agent_id, false).await;
            if !quiet {
                println!("❌ Agent {} failed: {}", agent_id, e);
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    // Store result
    let result = AgentResult {
        agent_id,
        success,
        output,
        duration_ms,
        tools_used,
    };
    {
        let mut r = results.write().await;
        r.push(result);
    }

    // Unregister from health monitor
    monitor.unregister_agent(agent_id).await;

    drop(permit);
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let level = if args.quiet { "warn" } else { "info" };
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)))
        .init();

    match args.command {
        Commands::Run(run_args) => run_swarm(run_args, args.quiet).await?,
        Commands::Ingest { run_id, file } => ingest::handle_ingest(&run_id, file).await?,
        Commands::Priorities { agent_type } => ingest::show_priorities(agent_type.as_deref()).await?,
        Commands::NextSteps { agent_type } => ingest::show_next_steps(agent_type.as_deref()).await?,
    }

    Ok(())
}

async fn run_swarm(args: RunArgs, quiet: bool) -> Result<()> {
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| default_model(args.provider).to_string());

    let cwd = args
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    if !quiet {
        println!("\n🐝 Gestalt Swarm v1.0 — RUN MODE");
        println!("   Goal: {}", args.goal);
        println!("   Agents: {}", args.agents);
        println!("   Max concurrency: {}", args.max_concurrency);
        println!("   Provider: {:?}", args.provider);
        println!("   Model: {}", model);
        println!("   CWD: {:?}\n", cwd);
    }

    // Health monitoring setup
    let health_config = HealthConfig::default();
    let monitor = Arc::new(SwarmHealthMonitor::new(health_config.clone()));

    // Spawn health checker
    let checker_monitor = monitor.clone();
    let checker_config = health_config.clone();
    tokio::spawn(async move {
        let checker = HealthChecker::new(checker_monitor, checker_config);
        checker.run().await;
    });

    // Shared state
    let semaphore = Arc::new(Semaphore::new(args.max_concurrency));
    let results: Arc<RwLock<Vec<AgentResult>>> = Arc::new(RwLock::new(Vec::new()));

    let start_time = Instant::now();

    // Spawn all agents concurrently (bounded by semaphore)
    let mut handles = Vec::with_capacity(args.agents);

    for agent_id in 0..args.agents {
        let goal = args.goal.clone();
        let cwd = cwd.clone();
        let sem = semaphore.clone();
        let res = results.clone();
        let prov = args.provider;
        let mdl = model.clone();
        let mon = monitor.clone();

        let handle = tokio::spawn(async move {
            run_agent(agent_id, goal, cwd, prov, mdl, sem, res, quiet, mon).await;
        });

        handles.push(handle);
    }

    // Wait for all agents to complete
    for handle in handles {
        if let Err(e) = handle.await {
            warn!("Agent task failed: {}", e);
        }
    }

    let total_duration_ms = start_time.elapsed().as_millis() as u64;

    // Shutdown health monitoring
    monitor.shutdown();

    // Report summary
    let all_results = results.read().await;
    let successes = all_results.iter().filter(|r| r.success).count();
    let failures = all_results.len() - successes;

    println!("\n{}", "=".repeat(60));
    println!("📊 SWARM SUMMARY");
    println!("{}", "=".repeat(60));
    println!("  Total agents: {}", all_results.len());
    println!("  ✅ Success: {}", successes);
    println!("  ❌ Failed: {}", failures);
    println!("  ⏱️  Total time: {}ms", total_duration_ms);
    println!(
        "  📈 Throughput: {:.1} agents/sec",
        all_results.len() as f64 / (total_duration_ms as f64 / 1000.0)
    );

    if !quiet {
        println!("\n{}", "-".repeat(60));
        println!("📋 HEALTH REPORT");
        println!("{}", "-".repeat(60));

        let all_health = monitor.get_all_health().await;
        for health in all_health {
            println!(
                "  Agent {} | {:?} | restarts: {} | fails: {}",
                health.agent_id, health.status, health.restart_count, health.consecutive_failures
            );
            if let Some(err) = health.last_error {
                println!("       └─ Last error: {}", err);
            }
        }

        println!("\n{}", "-".repeat(60));
        println!("📋 Agent Results:");
        println!("{}", "-".repeat(60));

        for result in all_results.iter() {
            let status = if result.success { "✅" } else { "❌" };
            println!(
                "  Agent {} | {} | {}ms | tools:{}",
                result.agent_id, status, result.duration_ms, result.tools_used
            );
            if result.success {
                let preview = result.output.chars().take(120).collect::<String>();
                println!("       └─ {}", preview);
            } else {
                println!("       └─ {}", result.output);
            }
        }
    }

    println!("\n{}", "=".repeat(60));

    // Exit with error if any agent failed
    if failures > 0 {
        std::process::exit(1);
    }

    Ok(())
}
