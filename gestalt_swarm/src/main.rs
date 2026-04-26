use anyhow::Result;
use clap::{Parser, ValueEnum, ValueHint};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod health;
mod shared;

use gestalt_core::application::agent::tools::{AskAiTool, ExecuteShellTool, GitStatusTool};
use health::{
    HealthChecker, HealthConfig, HealthEvent, RecoveryAction, RecoveryActionType,
    RecoveryManager, SwarmHealthMonitor,
};
use shared::SharedState;
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

    /// Be quiet (less output)
    #[arg(short, long)]
    quiet: bool,

    /// Enable health monitoring
    #[arg(long, default_value = "true")]
    health_monitoring: bool,

    /// Enable automatic recovery for failed agents
    #[arg(long, default_value = "true")]
    auto_recovery: bool,

    /// Maximum restart attempts per agent
    #[arg(long, default_value = "3")]
    max_restart_attempts: u64,

    /// Heartbeat interval in milliseconds
    #[arg(long, default_value = "5000")]
    heartbeat_interval_ms: u64,

    /// Maximum missed heartbeats before marking unhealthy
    #[arg(long, default_value = "15000")]
    max_heartbeat_delay_ms: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum LlmProviderKind {
    Gemini,
    Groq,
    Minimax,
}

// ============================================================================
// Shared Result Collection
// ============================================================================

/// Thread-safe collection of agent results
#[derive(Debug, Default, Clone)]
struct AgentResultSet(SharedState<Vec<AgentResult>>);

impl AgentResultSet {
    fn new() -> Self {
        Self(SharedState::default())
    }

    async fn push(&self, result: AgentResult) {
        self.0.write().await.push(result);
    }

    async fn all(&self) -> Vec<AgentResult> {
        self.0.read().await.clone()
    }

    async fn success_count(&self) -> usize {
        self.0.read().await.iter().filter(|r| r.success).count()
    }
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
            let api_key =
                std::env::var("GEMINI_API_KEY").map_err(|_| {
                    anyhow::anyhow!("GEMINI_API_KEY is required for --provider gemini")
                })?;
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
// Agent Context
// ============================================================================

/// Bundles all data needed to run a single agent
struct AgentContext {
    agent_id: usize,
    goal: String,
    cwd: PathBuf,
    provider: LlmProviderKind,
    model: String,
    semaphore: Arc<Semaphore>,
    results: AgentResultSet,
    quiet: bool,
    monitor: Option<Arc<SwarmHealthMonitor>>,
    heartbeat_interval_ms: u64,
}

impl AgentContext {
    fn run(self) -> AgentResult {
        let start = Instant::now();
        let rt = tokio::runtime::Handle::current();

        // Run the async agent in a blocking manner for simplicity
        rt.block_on(self.run_async(start))
    }

    async fn run_async(self, start: Instant) -> AgentResult {
        let permit = self.semaphore.acquire().await.expect("semaphore poisoned");

        if !self.quiet {
            println!("🟢 Agent {} started (cwd: {:?})", self.agent_id, self.cwd);
        }

        // Health monitoring lifecycle
        if let Some(ref monitor) = self.monitor {
            monitor.register_agent(self.agent_id).await;
            let mon = monitor.clone();
            let id = self.agent_id;
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_millis(5000));
                loop {
                    interval.tick().await;
                    mon.heartbeat(id).await;
                }
            });
        }

        let result = self.execute_llm().await;

        // Cleanup
        if let Some(ref monitor) = self.monitor {
            monitor.unregister_agent(self.agent_id).await;
        }

        drop(permit);
        result
    }

    async fn execute_llm(self) -> AgentResult {
        let start = Instant::now();
        let mut tools_used = 0;

        let llm = match build_llm_provider(self.provider, self.model.clone()) {
            Ok(provider) => provider,
            Err(e) => {
                if let Some(ref monitor) = self.monitor {
                    monitor
                        .report_error(self.agent_id, e.to_string())
                        .await;
                    monitor.report_task_complete(self.agent_id, false).await;
                }
                return AgentResult {
                    agent_id: self.agent_id,
                    success: false,
                    output: format!("Agent {} failed before LLM call: {}", self.agent_id, e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    tools_used: 0,
                };
            }
        };

        let registry = ToolRegistry::new();
        registry.register_tool(ExecuteShellTool).await;
        registry.register_tool(GitStatusTool).await;
        registry
            .register_tool(AskAiTool {
                llm_provider: llm.clone(),
            })
            .await;

        let prompt = format!(
            "[Agent {}] Task: {}\n\
            Working directory: {:?}\n\
            Provider: {:?}\n\
            Model: {}\n\
            Execute this task and report results concisely.\n\
            Use tools: execute_shell, git_status, ask_ai\n",
            self.agent_id, self.goal, self.cwd, self.provider, self.model
        );

        match llm.generate(&prompt).await {
            Ok(response) => {
                tools_used = 1;
                if let Some(ref monitor) = self.monitor {
                    monitor.report_task_complete(self.agent_id, true).await;
                }
                if !self.quiet {
                    println!("✅ Agent {} completed successfully", self.agent_id);
                }
                AgentResult {
                    agent_id: self.agent_id,
                    success: true,
                    output: response,
                    duration_ms: start.elapsed().as_millis() as u64,
                    tools_used,
                }
            }
            Err(e) => {
                if let Some(ref monitor) = self.monitor {
                    monitor.report_error(self.agent_id, e.to_string()).await;
                    monitor.report_task_complete(self.agent_id, false).await;
                }
                if !self.quiet {
                    println!("❌ Agent {} failed: {}", self.agent_id, e);
                }
                AgentResult {
                    agent_id: self.agent_id,
                    success: false,
                    output: format!("Agent {} failed: {}", self.agent_id, e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    tools_used,
                }
            }
        }
    }
}

// ============================================================================
// Swarm Execution
// ============================================================================

async fn run_swarm(args: &Cli, model: String, cwd: PathBuf) -> anyhow::Result<AgentResultSet> {
    let semaphore = Arc::new(Semaphore::new(args.max_concurrency));
    let results = AgentResultSet::new();

    // Health monitoring setup
    let monitor = if args.health_monitoring {
        let config = HealthConfig {
            heartbeat_interval_ms: args.heartbeat_interval_ms,
            health_check_interval_ms: 2000,
            max_heartbeat_delay_ms: args.max_heartbeat_delay_ms,
            max_restart_attempts: args.max_restart_attempts,
            recovery_delay_ms: 1000,
            enable_auto_recovery: args.auto_recovery,
        };
        let mon = Arc::new(SwarmHealthMonitor::new(config.clone()));

        // Spawn background tasks
        let checker = HealthChecker::new(mon.clone(), config.clone());
        tokio::spawn(async move {
            checker.run().await;
        });

        if args.auto_recovery {
            let (recovery_tx, mut recovery_rx) = mpsc::channel::<RecoveryAction>(100);
            let recovery_manager =
                RecoveryManager::new(mon.clone(), config, recovery_tx);
            tokio::spawn(async move {
                recovery_manager.run().await;
            });
            tokio::spawn(async move {
                while let Some(action) = recovery_rx.recv().await {
                    match action.action {
                        RecoveryActionType::Restart => {
                            info!("Restarting agent {}", action.agent_id);
                        }
                        RecoveryActionType::Reset => {
                            info!("Resetting agent {}", action.agent_id);
                        }
                        RecoveryActionType::Kill => {
                            warn!("Killing agent {}", action.agent_id);
                        }
                    }
                }
            });
        }

        Some(mon)
    } else {
        None
    };

    // Spawn all agents
    let mut handles = Vec::with_capacity(args.agents);
    for agent_id in 0..args.agents {
        let ctx = AgentContext {
            agent_id,
            goal: args.goal.clone(),
            cwd: cwd.clone(),
            provider: args.provider,
            model: model.clone(),
            semaphore: semaphore.clone(),
            results: results.clone(),
            quiet: args.quiet,
            monitor: monitor.clone(),
            heartbeat_interval_ms: args.heartbeat_interval_ms,
        };

        handles.push(tokio::spawn(async move {
            ctx.run()
        }));
    }

    for handle in handles {
        if let Err(e) = handle.await {
            warn!("Agent task failed: {}", e);
        }
    }

    if let Some(ref mon) = monitor {
        mon.shutdown();
    }

    Ok(results)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| default_model(args.provider).to_string());

    let level = if args.quiet { "warn" } else { "info" };
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)))
        .init();

    let cwd = args
        .cwd
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    println!("\n🐝 Gestalt Swarm v1.0");
    println!("   Goal: {}", args.goal);
    println!("   Agents: {}", args.agents);
    println!("   Max concurrency: {}", args.max_concurrency);
    println!("   Provider: {:?}", args.provider);
    println!("   Model: {}", model);
    println!("   CWD: {:?}", cwd);
    println!("   Health Monitoring: {}", if args.health_monitoring { "ON" } else { "OFF" });
    println!("   Auto Recovery: {}", if args.auto_recovery { "ON" } else { "OFF" });
    println!();

    let start_time = Instant::now();
    let results = run_swarm(&args, model, cwd).await?;
    let total_duration_ms = start_time.elapsed().as_millis() as u64;

    let all_results = results.all().await;
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

    if !args.quiet {
        println!("\n{}", "-".repeat(60));
        println!("📋 Agent Results:");
        println!("{}", "-".repeat(60));

        for result in &all_results {
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

    if failures > 0 {
        std::process::exit(1);
    }

    Ok(())
}
