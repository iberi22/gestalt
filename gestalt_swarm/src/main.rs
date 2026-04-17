use anyhow::Result;
use clap::{Parser, ValueHint};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Semaphore, RwLock};
use tracing::warn;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use gestalt_core::application::agent::tools::{
    AskAiTool, ExecuteShellTool, GitStatusTool,
};
use synapse_agentic::prelude::{
    MinimaxProvider, ToolRegistry, LLMProvider,
};

// ============================================================================
// CLI
// ============================================================================

#[derive(Parser, Debug)]
#[command(name = "swarm")]
#[command(about = "🐝 Gestalt Swarm — Parallel Agent Execution", long_about = None)]
struct Cli {
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

    /// Model to use
    #[arg(long, default_value = "MiniMax-Text-01")]
    model: String,

    /// Be quiet (less output)
    #[arg(short, long)]
    quiet: bool,
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

// ============================================================================
// Swarm Execution
// ============================================================================

async fn run_agent(
    agent_id: usize,
    goal: String,
    cwd: PathBuf,
    model: String,
    semaphore: Arc<Semaphore>,
    results: Arc<RwLock<Vec<AgentResult>>>,
    quiet: bool,
) {
    let start = Instant::now();
    let permit = semaphore.acquire().await.unwrap();

    if !quiet {
        println!("🟢 Agent {} started (cwd: {:?})", agent_id, cwd);
    }

    let mut tools_used = 0;
    let mut success = false;
    let mut output = String::new();

    // Initialize LLM provider
    let api_key = std::env::var("MINIMAX_API_KEY").unwrap_or_default();
    let group_id = std::env::var("MINIMAX_GROUP_ID").unwrap_or_default();

    let llm = Arc::new(MinimaxProvider::new(
        api_key.clone(),
        group_id.clone(),
        model.clone(),
    ));

    // Build minimal tool registry for this agent
    let registry = ToolRegistry::new();
    registry.register_tool(ExecuteShellTool).await;
    registry.register_tool(GitStatusTool).await;
    registry.register_tool(AskAiTool { llm_provider: llm.clone() }).await;

    // Execute the goal
    let prompt = format!(
        "[Agent {}] Task: {}\n\
        Working directory: {:?}\n\
        Execute this task and report results concisely.\n\
        Use tools: execute_shell, git_status, ask_ai\n",
        agent_id, goal, cwd
    );

    match llm.generate(&prompt).await {
        Ok(response) => {
            success = true;
            output = response;
            tools_used = 1;
            if !quiet {
                println!("✅ Agent {} completed successfully", agent_id);
            }
        }
        Err(e) => {
            success = false;
            output = format!("Agent {} failed: {}", agent_id, e);
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

    drop(permit);
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let level = if args.quiet { "warn" } else { "info" };
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)))
        .init();

    let cwd = args.cwd.unwrap_or_else(|| std::env::current_dir().unwrap());

    println!("\n🐝 Gestalt Swarm v1.0");
    println!("   Goal: {}", args.goal);
    println!("   Agents: {}", args.agents);
    println!("   Max concurrency: {}", args.max_concurrency);
    println!("   Model: {}", args.model);
    println!("   CWD: {:?}\n", cwd);

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
        let quiet = args.quiet;
        let model = args.model.clone();

        let handle = tokio::spawn(async move {
            run_agent(agent_id, goal, cwd, model, sem, res, quiet).await;
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
    println!("  📈 Throughput: {:.1} agents/sec",
        all_results.len() as f64 / (total_duration_ms as f64 / 1000.0));

    if !args.quiet {
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