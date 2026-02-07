use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use std::io::Write;
use std::path::Path;
use gestalt_timeline::cli::{Cli, Commands, repl};
use gestalt_timeline::db::SurrealClient;
use gestalt_timeline::config::Settings;
use gestalt_timeline::services::{
    AgentService, OrchestrationAction, ProjectService,
    TaskService, TimelineService, WatchService, AgentRuntime, start_server, AuthService,
    TelegramService, Cognition
};
use gestalt_timeline::services::llm_minimax::{LLMService as MiniMaxLLMService, LLMResponse};
#[cfg(feature = "bedrock")]
use gestalt_timeline::services::llm_bedrock::LLMService as BedrockLLMService;
use gestalt_timeline::services::GeminiService;
use gestalt_core::context::{detector, scanner};
use surrealdb::sql::Thing;
use std::sync::Arc;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Re-export specific models if needed for matching
use gestalt_timeline::models::{TaskStatus};

/// Helper to convert Option<Thing> to String for display
fn thing_to_string(thing: &Option<Thing>) -> String {
    thing.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "unknown".to_string())
}

/// Helper to initialize cognition service based on configuration
async fn init_cognition(
    db: &SurrealClient,
    timeline: &TimelineService,
    settings: &gestalt_timeline::config::CognitionSettings,
) -> Result<Arc<dyn Cognition>> {
    let provider = settings.provider.to_lowercase();
    let model_id = &settings.model_id;

    match provider.as_str() {
        "minimax" => {
            info!("🚀 Initializing MiniMax cognition service...");
            let llm_service = MiniMaxLLMService::new(
                db.clone(),
                timeline.clone(),
                Some(model_id.clone()),
            ).await?;
            Ok(Arc::new(llm_service))
        }
        "gemini" => {
            info!("🚀 Initializing Gemini cognition service...");
            let api_key = settings.gemini_api_key.clone()
                .or_else(|| std::env::var("GEMINI_API_KEY").ok());

            Ok(Arc::new(GeminiService::new(
                db.clone(),
                timeline.clone(),
                model_id.clone(),
                api_key,
            )?))
        }
        #[cfg(feature = "bedrock")]
        "bedrock" | "claude" | "anthropic" => {
            info!("🚀 Initializing Bedrock/Claude cognition service...");
            let llm_service = BedrockLLMService::new(
                db.clone(),
                timeline.clone(),
                settings,
            ).await?;
            Ok(Arc::new(llm_service))
        }
        _ => {
            // Default to MiniMax
            info!("🚀 Unknown provider '{}', defaulting to MiniMax...", provider);
            let llm_service = MiniMaxLLMService::new(
                db.clone(),
                timeline.clone(),
                Some(model_id.clone()),
            ).await?;
            Ok(Arc::new(llm_service))
        }
    }
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
        context_str.push_str(&format!("--- File: {} ---\n{}\n\n", file.path, file.content));
    }

    // Truncate if too long (approx 16k chars ~ 4k tokens)
    if context_str.len() > 16000 {
         context_str.truncate(16000);
         context_str.push_str("\n... (truncated context)");
    }

    info!("🧠 Context Engine: Added {} chars of context.", context_str.len());
    context_str
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
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
        // Initialize cognition service
        let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

        let mut final_prompt = prompt.clone();
        if cli.context {
            let context_str = collect_context(Path::new("."));
            final_prompt = format!("CONTEXT:\n{}\n\nUSER PROMPT:\n{}", context_str, prompt);
        }

        println!("🤖 Sending message to {}...", cognition.model_id());
        let response = cognition.chat(&agent_id, &final_prompt).await?;

        if cli.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("\n💬 {}:\n{}", cognition.model_id(), response.content);
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
                println!("✅ Project created: {} (ID: {})", project.name, thing_to_string(&project.id));
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

        Some(Commands::AddTask { project, description }) => {
            let task = task_service.create_task(&project, &description, &agent_id).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!("✅ Task created: {} (ID: {})", task.description, task.id.as_ref().map(|x| x.to_string()).unwrap_or_else(|| "none".to_string()));
            }
        }

        Some(Commands::UpdateTask { id, description, status }) => {
            let task_status = if let Some(s) = status {
                 match s.to_lowercase().as_str() {
                     "todo" => Some(TaskStatus::Pending),
                     "inprogress" | "running" => Some(TaskStatus::Running),
                     "completed" | "done" => Some(TaskStatus::Completed),
                     "cancelled" | "failed" => Some(TaskStatus::Cancelled), // Mapping failed/cancelled together roughly or user choice
                     _ => {
                         eprintln!("Unknown status: {}. Valid: todo, running, completed, cancelled", s);
                         return Ok(());
                     }
                 }
            } else {
                None
            };

            let task = task_service.update_task(&id, description, task_status, &agent_id).await?;
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
                    eprintln!("Invalid time format. Please use ISO 8601 (e.g., 2023-10-27T10:00:00Z)");
                    return Ok(());
                }
            };

            let task = task_service.schedule_task(&id, execute_at, &agent_id).await?;
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
            } else {
                if projects.is_empty() {
                    println!("📋 No projects found.");
                } else {
                    println!("📋 Projects:");
                    for p in projects {
                        println!("  • {} [{}] - {}", p.name, p.status, thing_to_string(&p.id));
                    }
                }
            }
        }

        Some(Commands::ListTasks { project }) => {
            let tasks = task_service.list_tasks(project.as_deref()).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&tasks)?);
            } else {
                if tasks.is_empty() {
                    println!("📋 No tasks found.");
                } else {
                    println!("📋 Tasks:");
                    for t in tasks {
                        println!("  • [{}] {} - {}", t.status, t.description, t.id.as_ref().map(|x| x.to_string()).unwrap_or_else(|| "none".to_string()));
                    }
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
                println!("   Tasks: {} total, {} completed", status.total_tasks, status.completed_tasks);
                println!("   Progress: {}%", status.progress_percent);
            }
        }

        Some(Commands::Timeline { since }) => {
            let events = timeline_service.get_timeline(since.as_deref()).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else {
                if events.is_empty() {
                    println!("🕐 No events in timeline.");
                } else {
                    println!("🕐 Timeline:");
                    for e in events {
                        println!("  {} | {} | {} | {}",
                            e.timestamp,
                            e.agent_id,
                            e.event_type,
                            e.id.as_ref().map(|x| x.to_string()).unwrap_or_else(|| "none".to_string())
                        );
                    }
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
                println!("📡 Subscribed to project: {} ({})", project, thing_to_string(&p.id));

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

        Some(Commands::AgentDisconnect {}) => {
            agent_service.disconnect(&agent_id).await?;
            if cli.json {
                println!(r#"{{"agent_id": "{}", "status": "disconnected"}}"#, agent_id);
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
            } else {
                if agents.is_empty() {
                    println!("🤖 No agents found.");
                } else {
                    println!("🤖 Agents:");
                    for a in agents {
                        println!("  • {} [{}] ({}) - last seen: {}",
                            a.name,
                            a.status,
                            a.agent_type,
                            a.last_seen
                        );
                    }
                }
            }
        }

        Some(Commands::AiChat { message }) => {
            // Initialize cognition service
            let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

            println!("🤖 Sending message to {}...", cognition.model_id());
            let response = cognition.chat(&agent_id, &message).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("\n💬 {}:\n{}", cognition.model_id(), response.content);
                println!("\n📊 Tokens: {} in / {} out", response.input_tokens, response.output_tokens);
            }
        }

        Some(Commands::AiOrchestrate { workflow, project: _, dry_run: _ }) => {
            // Initialize cognition service
            let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

            // Initialize Agent Runtime
            let runtime = AgentRuntime::new(
                agent_id.clone(),
                cognition,
                project_service.clone(),
                task_service.clone(),
                watch_service.clone(),
                agent_service.clone(),
            );

            println!("🔄 Starting Autonomous Agent Loop: '{}'", workflow);

            match runtime.run_loop(&workflow).await {
                Ok(_) => println!("\n✅ Autonomous Goal Completed."),
                Err(e) => println!("\n❌ Agent Error: {:?}", e),
            }
        }

        Some(Commands::Server { port }) => {
            // Initialize cognition service
            let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

            // Initialize Agent Runtime
            let runtime = AgentRuntime::new(
                agent_id.clone(),
                cognition,
                project_service.clone(),
                task_service.clone(),
                watch_service.clone(),
                agent_service.clone(),
            );

            start_server(runtime, timeline_service.clone(), agent_service.clone(), project_service.clone(), task_service.clone(), port).await?;
        }

        Some(Commands::Login {}) => {
            let auth = AuthService::new()?;

            // Check if we already have a valid token
            if auth.get_valid_token().await.is_ok() {
                println!("✅ Already logged in (valid credential found).");
                println!("💡 To force re-login, delete ~/.gestalt/gemini_credentials.json manually.");
                return Ok(());
            }

            println!("🔑 Starting Google Gemini OAuth2 login...");
            match auth.login().await {
                Ok(creds) => println!("✅ Login successful! Token expires at: {:?}", creds.expires_at),
                Err(e) => println!("❌ Login failed: {:?}", e),
            }
        }

        Some(Commands::Chat {}) => {
             // Initialize cognition service
            let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

            // Run REPL
            repl::run_repl(&agent_id, cognition).await?;
        }

        Some(Commands::IndexRepo { url }) => {
            // Check mode before write operation
            if cli.mode.to_lowercase() == "plan" {
                eprintln!("❌ Cannot index repository in 'plan' mode. Use '--mode build' for write operations.");
                return Ok(());
            }

            println!("📥 Indexing repository: {}", url);
            // Placeholder for actual indexing logic via AgentOrchestrator
            // In production, this would call gestalt_core::application::agent::AgentOrchestrator::index_repo
            println!("⚠️ Note: Full RAG indexing not yet implemented. This is a placeholder.");
            if cli.json {
                println!(r#"{{"status": "pending", "url": "{}"}}"#, url);
            } else {
                println!("✅ Repository queued for indexing: {}", url);
            }
        }

        Some(Commands::Bot) => {
            let telegram_settings = settings.telegram.ok_or_else(|| anyhow::anyhow!("Telegram settings not configured"))?;

            // Initialize cognition service
            let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

            let bot_service = TelegramService::new(
                telegram_settings.bot_token,
                cognition,
                telegram_settings.allowed_users,
            );

            bot_service.start().await?;
        }

        None => {
             // No command provided. If prompt is also None (checked above), show help or REPL
             // But we handled prompt above. So if we are here, prompt was None and command was None.
             // Start REPL in that case.

             // Initialize cognition service
            let cognition = init_cognition(&db, &timeline_service, &settings.cognition).await?;

            // Run REPL
            repl::run_repl(&agent_id, cognition).await?;
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
