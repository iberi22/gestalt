use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use gestalt_core::application::agent::tools::{ExecuteShellTool, ReadFileTool, WriteFileTool};
use gestalt_timeline::cli::{repl, AgentCommands, Cli, Commands};
use gestalt_timeline::config::Settings;
use gestalt_timeline::db::SurrealClient;
#[cfg(feature = "telegram")]
use gestalt_timeline::services::TelegramService;
use gestalt_timeline::services::{
    start_server, AgentRuntime, AgentService, AuthService, DispatcherService, IndexService,
    MemoryService, ProjectService, ProtocolSyncService, QueuedTask, TaskQueue, TaskService,
    TaskSource, TimelineService, WatchService,
};
use std::path::Path;

use gestalt_core::context::{detector, scanner};
use std::sync::Arc;
use surrealdb::sql::Thing;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Re-export specific models if needed for matching
use gestalt_timeline::models::TaskStatus;

/// Helper to convert Option<Thing> to String for display
fn thing_to_string(thing: &Option<Thing>) -> String {
    thing
        .as_ref()
        .map(|t| t.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

use synapse_agentic::prelude::*;

/// Helper to initialize decision engine based on configuration
async fn init_decision_engine(
    settings: &gestalt_timeline::config::CognitionSettings,
) -> Result<Arc<DecisionEngine>> {
    let provider_name = settings.provider.to_lowercase();
    let model_id = &settings.model_id;

    // Use framework's resilience components for transparent failover
    let store = Arc::new(InMemoryCooldownStore::new());
    let mut rotator = StochasticRotator::new(store);
    let mut providers_added = 0usize;

    // Primary provider implementation
    if provider_name == "minimax" || provider_name == "auto" {
        if let Some(api_key) = settings
            .minimax_api_key
            .clone()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
        {
            info!("🚀 Initializing MiniMax resilient provider...");
            let group_id = std::env::var("MINIMAX_GROUP_ID").unwrap_or_default();
            let provider = MinimaxProvider::new(api_key, group_id, model_id.clone());
            rotator.add_provider(ProviderId::new("minimax", model_id), Arc::new(provider));
            providers_added += 1;
        }
    }

    if provider_name == "gemini" || provider_name == "auto" {
        if let Some(api_key) = settings
            .gemini_api_key
            .clone()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        {
            info!("🚀 Initializing Gemini resilient provider...");
            let provider = GeminiProvider::new(api_key, model_id.clone());
            rotator.add_provider(ProviderId::new("gemini", model_id), Arc::new(provider));
            providers_added += 1;
        }
    }

    // Secondary fallback configurations (cross-registering)
    if provider_name == "gemini" {
        if let Some(api_key) = settings
            .minimax_api_key
            .clone()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
        {
            let group_id = std::env::var("MINIMAX_GROUP_ID").unwrap_or_default();
            let provider = MinimaxProvider::new(api_key, group_id, model_id.clone());
            rotator.add_provider(
                ProviderId::new("minimax-fallback", model_id),
                Arc::new(provider),
            );
            providers_added += 1;
        }
    } else if provider_name == "minimax" {
        if let Some(api_key) = settings
            .gemini_api_key
            .clone()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        {
            let provider = GeminiProvider::new(api_key, model_id.clone());
            rotator.add_provider(
                ProviderId::new("gemini-fallback", model_id),
                Arc::new(provider),
            );
            providers_added += 1;
        }
    }

    if providers_added == 0 {
        warn!("No external LLM providers configured; using rule-based decision mode.");
        Ok(Arc::new(DecisionEngine::builder().build()))
    } else {
        // The engine now uses the stochastic rotator as its single entry point
        // providing transparent failover between all added providers.
        // It's crucial this is the FIRST provider added so AgentRuntime picks it up.
        Ok(Arc::new(
            DecisionEngine::builder().with_provider(rotator).build(),
        ))
    }
}

/// Initialize native tools for AgentRuntime.
async fn init_tool_registry(
    vector_db: Arc<dyn gestalt_core::ports::outbound::repo_manager::VectorDb>,
    embedding_model: Arc<dyn gestalt_core::domain::rag::embeddings::EmbeddingModel>,
) -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    registry.register_tool(ExecuteShellTool).await;
    registry.register_tool(ReadFileTool).await;
    registry.register_tool(WriteFileTool).await;
    registry
        .register_tool(
            gestalt_core::application::agent::tools::SearchCodeTool::new(
                vector_db,
                embedding_model,
            ),
        )
        .await;
    registry
}

/// Collect context from the current directory
fn collect_context(root: &Path) -> String {
    info!("🧠 Context Engine: Analyzing project...");
    let project_type = detector::detect_project_type(root);
    let tree = scanner::generate_directory_tree(root, 2);
    let files = scanner::scan_markdown_files(root);

    let mut context_str = String::new();
    context_str.push_str(&format!("Project Type: {}\n", project_type));
    context_str.push_str("Directory Structure:\n");
    context_str.push_str(&tree);
    context_str.push_str("\nMarkdown Context (first 100 lines):\n");

    for file in files {
        context_str.push_str(&format!(
            "--- File: {} ---\n{}\n\n",
            file.path, file.content
        ));
    }

    // Truncate if too long (approx 16k chars ~ 4k tokens)
    if context_str.len() > 16000 {
        context_str.truncate(16000);
        context_str.push_str("\n... (truncated context)");
    }

    info!(
        "🧠 Context Engine: Added {} chars of context.",
        context_str.len()
    );
    context_str
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration
    let settings = Settings::new()?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize database connection
    let db = SurrealClient::connect(&settings.database).await?;
    let vector_db: Arc<dyn gestalt_core::ports::outbound::repo_manager::VectorDb> =
        Arc::new(db.clone());

    // Default lightweight embedding model for fast local builds.
    let embedding_model: Arc<dyn gestalt_core::domain::rag::embeddings::EmbeddingModel> = {
        #[cfg(feature = "rag-embeddings")]
        {
            let mut model =
                Arc::new(gestalt_core::domain::rag::embeddings::DummyEmbeddingModel::new(384))
                    as Arc<dyn gestalt_core::domain::rag::embeddings::EmbeddingModel>;

            if std::path::Path::new("models/bert/config.json").exists()
                && std::path::Path::new("models/bert/tokenizer.json").exists()
                && std::path::Path::new("models/bert/model.safetensors").exists()
            {
                info!("🚀 Initializing BertEmbeddingModel...");
                match gestalt_infra_embeddings::BertEmbeddingModel::new(
                    "models/bert/config.json",
                    "models/bert/tokenizer.json",
                    "models/bert/model.safetensors",
                ) {
                    Ok(m) => model = Arc::new(m),
                    Err(e) => warn!(
                        "Failed to load BERT model: {}. Falling back to DummyEmbeddingModel.",
                        e
                    ),
                }
            } else {
                info!("BERT model files not found. Using DummyEmbeddingModel for RAG.");
            }
            model
        }
        #[cfg(not(feature = "rag-embeddings"))]
        {
            info!("Feature 'rag-embeddings' disabled. Using DummyEmbeddingModel for RAG.");
            Arc::new(gestalt_core::domain::rag::embeddings::DummyEmbeddingModel::new(384))
        }
    };

    // Initialize services
    let timeline_service = TimelineService::new(db.clone());
    let project_service = ProjectService::new(db.clone(), timeline_service.clone());
    let task_service = TaskService::new(db.clone(), timeline_service.clone());
    let watch_service = WatchService::new(db.clone(), timeline_service.clone());
    let agent_service = AgentService::new(db.clone(), timeline_service.clone());

    // Get agent ID from configuration
    let agent_id = settings.agent.id.clone();

    // Check if we have a direct prompt (Context Engine Mode)
    if let Some(prompt) = &cli.prompt {
        // Initialize decision engine
        let engine = init_decision_engine(&settings.cognition).await?;

        let mut final_prompt = prompt.clone();
        if cli.context {
            let context_str = collect_context(Path::new("."));
            final_prompt = format!("CONTEXT:\n{}\n\nUSER PROMPT:\n{}", context_str, prompt);
        }

        println!("🤖 Sending message to Decision Engine...");
        let context = DecisionContext::new("cli").with_summary(&final_prompt);

        let decision = engine.decide(&context).await?;

        if cli.json {
            let json_resp = serde_json::json!({
                "action": decision.action,
                "reasoning": decision.reasoning,
                "confidence": decision.confidence,
                "providers": decision.providers_used
            });
            println!("{}", serde_json::to_string_pretty(&json_resp)?);
        } else {
            println!("\n💬 Decision Engine:");
            println!("Action: {}", decision.action);
            println!("Reasoning: {}", decision.reasoning);
        }
        return Ok(());
    }

    // Handle subcommands
    match cli.command {
        Some(Commands::AddProject { name }) => {
            let project = project_service.create_project(&name, &agent_id).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&project)?);
            } else {
                println!(
                    "✅ Project created: {} (ID: {})",
                    project.name,
                    thing_to_string(&project.id)
                );
            }
        }

        Some(Commands::DeleteProject { id }) => {
            project_service.delete_project(&id, &agent_id).await?;
            if cli.json {
                println!("{{ \"status\": \"deleted\", \"id\": \"{}\" }}", id);
            } else {
                println!("🗑️ Project deleted: {}", id);
            }
        }

        Some(Commands::AddTask {
            project,
            description,
        }) => {
            let task = task_service
                .create_task(&project, &description, &agent_id)
                .await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!(
                    "✅ Task created: {} (ID: {})",
                    task.description,
                    task.id
                        .as_ref()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "none".to_string())
                );
            }
        }

        Some(Commands::UpdateTask {
            id,
            description,
            status,
        }) => {
            let task_status = if let Some(s) = status {
                match s.to_lowercase().as_str() {
                    "todo" => Some(TaskStatus::Pending),
                    "inprogress" | "running" => Some(TaskStatus::Running),
                    "completed" | "done" => Some(TaskStatus::Completed),
                    "cancelled" | "failed" => Some(TaskStatus::Cancelled), // Mapping failed/cancelled together roughly or user choice
                    _ => {
                        eprintln!(
                            "Unknown status: {}. Valid: todo, running, completed, cancelled",
                            s
                        );
                        return Ok(());
                    }
                }
            } else {
                None
            };

            let task = task_service
                .update_task(&id, description, task_status, &agent_id)
                .await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!("✏️ Task updated: {}", id);
            }
        }

        Some(Commands::DeleteTask { id }) => {
            task_service.delete_task(&id, &agent_id).await?;
            if cli.json {
                println!("{{ \"status\": \"deleted\", \"id\": \"{}\" }}", id);
            } else {
                println!("🗑️ Task deleted: {}", id);
            }
        }

        Some(Commands::ScheduleTask { id, time }) => {
            // Simple parsing for now - assumes ISO 8601 or we can try flexible parsing if needed
            // For MVP, strict ISO or fail
            let execute_at = match DateTime::parse_from_rfc3339(&time) {
                Ok(dt) => dt.with_timezone(&Utc),
                Err(_) => {
                    eprintln!(
                        "Invalid time format. Please use ISO 8601 (e.g., 2023-10-27T10:00:00Z)"
                    );
                    return Ok(());
                }
            };

            let task = task_service
                .schedule_task(&id, execute_at, &agent_id)
                .await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!("⏰ Task scheduled: {} at {}", id, execute_at);
            }
        }

        Some(Commands::RunTask { task_id }) => {
            let result = task_service.run_task(&task_id, &agent_id).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🚀 Task {} completed: {:?}", task_id, result.status);
            }
        }

        Some(Commands::ListProjects) => {
            let projects = project_service.list_projects().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&projects)?);
            } else if projects.is_empty() {
                println!("📋 No projects found.");
            } else {
                println!("📋 Projects:");
                for p in projects {
                    println!("  • {} [{}] - {}", p.name, p.status, thing_to_string(&p.id));
                }
            }
        }

        Some(Commands::ListTasks { project }) => {
            let tasks = task_service.list_tasks(project.as_deref()).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&tasks)?);
            } else if tasks.is_empty() {
                println!("📋 No tasks found.");
            } else {
                println!("📋 Tasks:");
                for t in tasks {
                    println!(
                        "  • [{}] {} - {}",
                        t.status,
                        t.description,
                        t.id.as_ref()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "none".to_string())
                    );
                }
            }
        }

        Some(Commands::Status { project }) => {
            let status = project_service.get_status(&project).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("📊 Project: {}", status.name);
                println!("   Status: {}", status.status);
                println!(
                    "   Tasks: {} total, {} completed",
                    status.total_tasks, status.completed_tasks
                );
                println!("   Progress: {}%", status.progress_percent);
            }
        }

        Some(Commands::Timeline { since }) => {
            let events = timeline_service.get_timeline(since.as_deref()).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else if events.is_empty() {
                println!("🕐 No events in timeline.");
            } else {
                println!("🕐 Timeline:");
                for e in events {
                    println!(
                        "  {} | {} | {} | {}",
                        e.timestamp,
                        e.agent_id,
                        e.event_type,
                        e.id.as_ref()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "none".to_string())
                    );
                }
            }
        }

        Some(Commands::Watch { project, events }) => {
            // Parse event filter
            let event_filter = events.map(|e| {
                e.split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>()
            });

            // Setup Ctrl+C handler
            let watch_service_clone = watch_service.clone();
            tokio::spawn(async move {
                tokio::signal::ctrl_c().await.ok();
                watch_service_clone.stop();
            });

            // Start watching (this blocks until stopped)
            watch_service
                .start_watching(&agent_id, project.as_deref(), event_filter)
                .await?;
        }

        Some(Commands::Broadcast { message, project }) => {
            let event = watch_service
                .broadcast_message(&agent_id, &message, project.as_deref())
                .await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&event)?);
            } else {
                println!("📢 Broadcast sent: {}", message);
            }
        }

        Some(Commands::Subscribe { project }) => {
            // Get project ID first
            let proj = project_service.get_by_name(&project).await?;
            if let Some(p) = proj {
                println!(
                    "📡 Subscribed to project: {} ({})",
                    project,
                    thing_to_string(&p.id)
                );

                // Setup Ctrl+C handler
                let watch_service_clone = watch_service.clone();
                tokio::spawn(async move {
                    tokio::signal::ctrl_c().await.ok();
                    watch_service_clone.stop();
                });

                // Start watching with project filter
                let project_id_str = thing_to_string(&p.id);
                watch_service
                    .start_watching(&agent_id, Some(&project_id_str), None)
                    .await?;
            } else {
                println!("❌ Project not found: {}", project);
            }
        }

        Some(Commands::AgentConnect { name }) => {
            let agent = agent_service.connect(&agent_id, name.as_deref()).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&agent)?);
            } else {
                println!("🤖 Agent connected: {} ({})", agent.name, agent.agent_type);
            }
        }

        Some(Commands::AgentDisconnect) => {
            agent_service.disconnect(&agent_id).await?;
            if cli.json {
                println!(
                    r#"{{"agent_id": "{}", "status": "disconnected"}}"#,
                    agent_id
                );
            } else {
                println!("👋 Agent disconnected: {}", agent_id);
            }
        }

        Some(Commands::ListAgents { online }) => {
            let agents = if online {
                agent_service.list_online_agents().await?
            } else {
                agent_service.list_agents().await?
            };
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&agents)?);
            } else if agents.is_empty() {
                println!("🤖 No agents found.");
            } else {
                println!("🤖 Agents:");
                for a in agents {
                    println!(
                        "  • {} [{}] ({}) - last seen: {}",
                        a.name, a.status, a.agent_type, a.last_seen
                    );
                }
            }
        }

        Some(Commands::AiChat { message }) => {
            // Initialize decision engine
            let engine = init_decision_engine(&settings.cognition).await?;

            println!("🤖 Sending message to Decision Engine...");
            let context = DecisionContext::new("chat").with_summary(&message);

            let decision = engine.decide(&context).await?;

            if cli.json {
                let json_resp = serde_json::json!({
                    "action": decision.action,
                    "reasoning": decision.reasoning,
                    "confidence": decision.confidence,
                    "providers": decision.providers_used
                });
                println!("{}", serde_json::to_string_pretty(&json_resp)?);
            } else {
                println!("\n💬 Decision Engine:");
                println!("Action: {}", decision.action);
                println!("Reasoning: {}", decision.reasoning);
            }
        }

        Some(Commands::AiOrchestrate {
            workflow,
            project: _,
            dry_run: _,
        }) => {
            // Initialize decision engine
            let engine = init_decision_engine(&settings.cognition).await?;
            let registry = init_tool_registry(vector_db.clone(), embedding_model.clone()).await;
            let memory_service = MemoryService::new(db.clone());

            // Initialize Agent Runtime
            let runtime = AgentRuntime::new(
                agent_id.clone(),
                engine,
                registry,
                project_service.clone(),
                task_service.clone(),
                timeline_service.clone(),
                watch_service.clone(),
                agent_service.clone(),
                memory_service,
            );

            println!("🔄 Starting Autonomous Agent Loop: '{}'", workflow);

            match runtime.run_loop(&workflow).await {
                Ok(_) => println!("\n✅ Autonomous Goal Completed."),
                Err(e) => println!("\n❌ Agent Error: {:?}", e),
            }
        }

        Some(Commands::ProtocolSync {
            project,
            path,
            to_db,
            to_markdown,
        }) => {
            let sync_service = ProtocolSyncService::new(db.clone(), timeline_service.clone());
            let path_buf = std::path::PathBuf::from(path);

            if to_db {
                println!(
                    "📥 Syncing from markdown to database for project: {}",
                    project
                );
                match sync_service
                    .sync_from_markdown(&path_buf, &project, &agent_id)
                    .await
                {
                    Ok(_) => println!("✅ Sync from markdown completed successfully."),
                    Err(e) => eprintln!("❌ Sync from markdown failed: {}", e),
                }
            }

            if to_markdown {
                println!(
                    "📤 Syncing from database to markdown for project: {}",
                    project
                );
                match sync_service.sync_to_markdown(&path_buf, &project).await {
                    Ok(_) => println!("✅ Sync to markdown completed successfully."),
                    Err(e) => eprintln!("❌ Sync to markdown failed: {}", e),
                }
            }

            if !to_db && !to_markdown {
                println!("💡 Please specify --to-db or --to-markdown (or both).");
            }
        }

        Some(Commands::Server { port }) => {
            // Initialize decision engine
            let engine = init_decision_engine(&settings.cognition).await?;
            let registry = init_tool_registry(vector_db.clone(), embedding_model.clone()).await;
            let memory_service = MemoryService::new(db.clone());

            // Initialize Agent Runtime
            let runtime = AgentRuntime::new(
                agent_id.clone(),
                engine,
                registry,
                project_service.clone(),
                task_service.clone(),
                timeline_service.clone(),
                watch_service.clone(),
                agent_service.clone(),
                memory_service,
            );

            start_server(
                runtime,
                timeline_service.clone(),
                agent_service.clone(),
                project_service.clone(),
                task_service.clone(),
                watch_service.clone(),
                port,
            )
            .await?;
        }

        Some(Commands::Login) => {
            let auth = AuthService::new()?;

            // Check if we already have a valid token
            if auth.get_valid_token().await.is_ok() {
                println!("✅ Already logged in (valid credential found).");
                println!(
                    "💡 To force re-login, delete ~/.gestalt/gemini_credentials.json manually."
                );
                return Ok(());
            }

            println!("🔑 Starting Google Gemini OAuth2 login...");
            match auth.login().await {
                Ok(creds) => println!(
                    "✅ Login successful! Token expires at: {:?}",
                    creds.expires_at
                ),
                Err(e) => println!("❌ Login failed: {:?}", e),
            }
        }

        Some(Commands::Chat) => {
            // Initialize decision engine
            let engine = init_decision_engine(&settings.cognition).await?;

            // Run REPL
            repl::run_repl(&agent_id, engine).await?;
        }

        Some(Commands::IndexRepo { url }) => {
            // Check mode before write operation
            if cli.mode.to_lowercase() == "plan" {
                eprintln!("❌ Cannot index repository in 'plan' mode. Use '--mode build' for write operations.");
                return Ok(());
            }

            println!("📥 Indexing repository: {}", url);
            let index_service = IndexService::new(db.clone(), embedding_model.clone());
            match index_service.index_repo(&url).await {
                Ok(_) => {
                    if cli.json {
                        println!(r#"{{"status": "completed", "url": "{}"}}"#, url);
                    } else {
                        println!("✅ Repository indexed successfully: {}", url);
                    }
                }
                Err(e) => {
                    if cli.json {
                        println!(
                            r#"{{"status": "error", "url": "{}", "error": "{}"}}"#,
                            url, e
                        );
                    } else {
                        eprintln!("❌ Failed to index repository {}: {}", url, e);
                    }
                }
            }
        }

        Some(Commands::Bot) => {
            #[cfg(feature = "telegram")]
            {
                let telegram_settings = settings
                    .telegram
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("Telegram settings not configured"))?;

                // Initialize decision engine
                let engine = init_decision_engine(&settings.cognition).await?;
                let watch_service =
                    Arc::new(WatchService::new(db.clone(), timeline_service.clone()));

                let bot_service = TelegramService::new(
                    telegram_settings.bot_token,
                    engine,
                    telegram_settings.allowed_users,
                    watch_service,
                    db.clone(),
                );

                bot_service.start().await?;
            }
            #[cfg(not(feature = "telegram"))]
            {
                anyhow::bail!(
                    "This binary was built without Telegram support. Rebuild with --features telegram."
                );
            }
        }

        Some(Commands::Nexus {
            workers,
            port,
            daemon,
        }) => {
            if daemon {
                println!("🚀 Starting Gestalt Nexus in background...");
                let mut cmd = std::process::Command::new(std::env::current_exe()?);

                // Reconstruct arguments, removing --daemon to avoid infinite loop
                let args: Vec<String> = std::env::args()
                    .skip(1)
                    .filter(|a| a != "--daemon" && !a.starts_with("--daemon="))
                    .collect();

                cmd.args(args);

                // Redirect stdout and stderr to gestalt-nexus.log
                let log_file = std::fs::File::create("gestalt-nexus.log")?;
                cmd.stdout(std::process::Stdio::from(log_file.try_clone()?));
                cmd.stderr(std::process::Stdio::from(log_file));

                #[cfg(windows)]
                {
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(CREATE_NO_WINDOW);
                }

                cmd.spawn()?;
                println!("✅ Nexus daemon spawned. Logs: gestalt-nexus.log");
                return Ok(());
            }

            info!("🚀 Starting Gestalt Nexus - Always-On Agentic Daemon");
            info!("   Workers: {}, API Port: {}", workers, port);

            // Initialize cognition
            let cognition = init_decision_engine(&settings.cognition).await?;
            let registry = init_tool_registry(vector_db.clone(), embedding_model.clone()).await;

            // Initialize memory service
            let _memory_service = Arc::new(MemoryService::new(db.clone()));

            // Create TaskQueue (buffer = 256 tasks)
            let (task_queue, task_receiver) = TaskQueue::new(db.clone(), 256);
            let task_queue = Arc::new(task_queue);

            // Wire Telegram bot to TaskQueue if configured
            #[cfg(feature = "telegram")]
            let tg_handle = if let Some(tg_settings) = settings.telegram.clone() {
                let tg_queue = Arc::clone(&task_queue);
                let tg_cognition = cognition.clone();
                let tg_watch = Arc::new(WatchService::new(db.clone(), timeline_service.clone()));
                let bot_service = TelegramService::new(
                    tg_settings.bot_token,
                    tg_cognition,
                    tg_settings.allowed_users,
                    tg_watch,
                    db.clone(),
                )
                .with_task_queue(tg_queue);

                info!("📡 Telegram bot configured with TaskQueue integration");
                Some(tokio::spawn(async move {
                    if let Err(e) = bot_service.start().await {
                        tracing::error!("Telegram bot error: {}", e);
                    }
                }))
            } else {
                info!("⚠️  Telegram not configured — skipping bot startup");
                None
            };
            #[cfg(not(feature = "telegram"))]
            let tg_handle: Option<tokio::task::JoinHandle<()>> = {
                if settings.telegram.is_some() {
                    info!(
                        "⚠️  Telegram settings present, but feature 'telegram' is disabled at compile-time."
                    );
                }
                None
            };

            // Recover any pending tasks from DB (handles restarts)
            let pending = task_queue.recover_pending().await.unwrap_or_default();
            for pending_task in pending {
                let _ = task_queue.enqueue(pending_task).await;
            }

            // Clone services for the factory closure
            let project_service_clone = project_service.clone();
            let task_service_clone = task_service.clone();
            let watch_service_clone = watch_service.clone();
            let agent_service_clone = agent_service.clone();
            let timeline_clone = timeline_service.clone();
            let timeline_for_api = timeline_clone.clone();

            // Start REST API server in background
            let memory_service = MemoryService::new(db.clone());
            let api_runtime = AgentRuntime::new(
                agent_id.clone(),
                cognition.clone(),
                registry.clone(),
                project_service.clone(),
                task_service.clone(),
                timeline_service.clone(),
                watch_service.clone(),
                agent_service.clone(),
                memory_service.clone(),
            );
            let api_handle = tokio::spawn(async move {
                if let Err(e) = start_server(
                    api_runtime,
                    timeline_for_api,
                    agent_service_clone,
                    project_service_clone,
                    task_service_clone,
                    watch_service_clone,
                    port,
                )
                .await
                {
                    tracing::error!("API server error: {}", e);
                }
            });

            // Launch the TaskQueue dispatch loop
            let tq_clone = Arc::clone(&task_queue);
            let tq_engine = cognition.clone();
            let tq_registry = registry.clone();
            let tq_project = project_service.clone();
            let tq_task = task_service.clone();
            let tq_watch = watch_service.clone();
            let tq_agent = agent_service.clone();

            info!(
                "⚙️  TaskQueue dispatch loop starting with {} max workers",
                workers
            );
            let tq_memory = memory_service.clone();
            tq_clone
                .run_dispatch_loop(task_receiver, workers, move |agent_id_str| {
                    let engine = tq_engine.clone();
                    let registry = tq_registry.clone();
                    let project = tq_project.clone();
                    let task = tq_task.clone();
                    let watch = tq_watch.clone();
                    let agent = tq_agent.clone();
                    let memory = tq_memory.clone();
                    let timeline = timeline_clone.clone();
                    async move {
                        Ok(AgentRuntime::new(
                            agent_id_str,
                            engine,
                            registry,
                            project,
                            task,
                            timeline,
                            watch,
                            agent,
                            memory,
                        ))
                    }
                })
                .await;

            // Wait for other services
            if let Some(h) = tg_handle {
                let _ = h.await;
            }
            let _ = api_handle.await;

            info!("🔴 Gestalt Nexus daemon shutting down.");
        }

        Some(Commands::Queue { goal, priority }) => {
            // Direct CLI task injection into the queue
            let (task_queue, _receiver) = TaskQueue::new(db.clone(), 1);
            let queued = QueuedTask::new(
                goal.clone(),
                TaskSource::Cli {
                    invocation: format!("gestalt queue '{}'", &goal[..goal.len().min(60)]),
                },
                priority,
            );
            task_queue.enqueue(queued).await?;
            if cli.json {
                println!(
                    r#"{{"status": "queued", "goal": "{}", "priority": {} }}"#,
                    goal, priority
                );
            } else {
                println!(
                    "📥 Task queued: '{}' (priority: {})",
                    &goal[..goal.len().min(80)],
                    priority
                );
                println!("   Start the Nexus daemon to process: gestalt nexus");
            }
        }

        Some(Commands::Agent { action }) => {
            let dispatcher = DispatcherService::new(Arc::new(timeline_service.clone()));

            match action {
                AgentCommands::Spawn { agent_type, prompt } => {
                    let task_name = dispatcher.spawn_agent(&agent_type, &prompt).await?;
                    if cli.json {
                        println!(
                            r#"{{"status": "spawned", "agent_type": "{}", "task_id": "{}"}}"#,
                            agent_type, task_name
                        );
                    } else {
                        println!(
                            "🚀 Spawned background agent: {} (Task ID: {})",
                            agent_type, task_name
                        );
                    }
                }
                AgentCommands::Ps => {
                    // MVP implementation
                    println!("📋 Agent process list (Check timeline for outputs)");
                }
            }
        }

        None => {
            // No command provided. If prompt is also None (checked above), show help or REPL
            // But we handled prompt above. So if we are here, prompt was None and command was None.
            // Start REPL in that case.

            // Initialize decision engine
            let engine = init_decision_engine(&settings.cognition).await?;

            // Run REPL
            repl::run_repl(&agent_id, engine).await?;
        }
    }

    // Explicitly drop services and database client for graceful shutdown
    // This helps minimize SurrealDB background task cancellation warnings
    drop(project_service);
    drop(task_service);
    drop(watch_service);
    drop(agent_service);
    drop(timeline_service);
    drop(db);

    Ok(())
}
