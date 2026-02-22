use clap::{Parser, Subcommand};
use dotenv::dotenv;
use futures::StreamExt;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use gestalt_core::adapters::auth::{google_oauth, qwen_oauth};
use gestalt_core::adapters::llm::gemini::GeminiProvider;
use gestalt_core::adapters::llm::gemini_oauth::GeminiOAuthProvider;
use gestalt_core::adapters::llm::ollama::OllamaProvider;
use gestalt_core::adapters::llm::openai::OpenAIProvider;
use gestalt_core::adapters::llm::qwen::QwenProvider;
use gestalt_core::application::config::AppConfig;
use gestalt_core::application::consensus::ConsensusService;
use gestalt_core::application::mcp_service::McpService;
use gestalt_core::context::{detector, scanner};
use gestalt_core::ports::outbound::llm_provider::{LlmProvider, LlmRequest};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// The prompt to send to the agents (when not using subcommands)
    #[arg(short, long)]
    prompt: Option<String>,

    /// Enable Multi-Model Consensus (default: false)
    #[arg(long, default_value_t = false)]
    consensus: bool,

    /// Enable Context Engine (default: true)
    #[arg(long, default_value_t = true)]
    context: bool,

    /// Google Gemini Model (default: gemini-2.0-flash)
    #[arg(long, default_value = "gemini-2.0-flash")]
    gemini_model: String,

    /// OpenAI Model (default: gpt-4)
    #[arg(long, default_value = "gpt-4")]
    openai_model: String,

    /// Qwen Model (default: qwen-coder)
    #[arg(long, default_value = "qwen-coder")]
    qwen_model: String,

    /// Ollama Model (default: llama2)
    #[arg(long, default_value = "llama2")]
    ollama_model: String,

    /// Ollama Base URL
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage configuration
    #[command(name = "config")]
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommands,
    },
    /// Login with Qwen OAuth2 (Device Code flow)
    #[command(name = "qwen-login")]
    QwenLogin,
    /// Logout from Qwen
    #[command(name = "qwen-logout")]
    QwenLogout,
    /// Show authentication status
    Status,
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// Initialize a default configuration file
    Init,
    /// Show current configuration
    Show,
}

#[tokio::main]
async fn main() {
    // 1. Initialize Logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // 2. Load Environment Variables (.env)
    dotenv().ok();

    // 3. Parse Args
    let cli = Cli::parse();

    // Handle subcommands
    match cli.command {
        Some(Commands::Config { subcommand }) => {
            match subcommand {
                ConfigCommands::Init => {
                    let default_config = AppConfig::default();
                    if let Some(path) = AppConfig::get_config_path() {
                        if path.exists() {
                            eprintln!("❌ Config file already exists at {:?}", path);
                            return;
                        }
                        if let Some(parent) = path.parent() {
                            std::fs::create_dir_all(parent)
                                .expect("Failed to create config directory");
                        }

                        let toml_str = toml::to_string_pretty(&default_config)
                            .expect("Failed to serialize config");
                        std::fs::write(&path, toml_str).expect("Failed to write config file");
                        println!("✅ Config file created at {:?}", path);
                    }
                }
                ConfigCommands::Show => match AppConfig::load() {
                    Ok(config) => println!("{:#?}", config),
                    Err(e) => eprintln!("❌ Failed to load config: {}", e),
                },
            }
            return;
        }
        Some(Commands::QwenLogin) => {
            match qwen_oauth::run_device_flow_login().await {
                Ok(_) => {
                    println!("✅ Qwen login successful!");
                }
                Err(e) => {
                    eprintln!("❌ Qwen login failed: {}", e);
                }
            }
            return;
        }
        Some(Commands::QwenLogout) => {
            match qwen_oauth::clear_credentials().await {
                Ok(_) => println!("✅ Qwen logged out successfully."),
                Err(e) => eprintln!("❌ Qwen logout failed: {}", e),
            }
            return;
        }
        Some(Commands::Status) => {
            println!("🔐 Authentication Status:\n");

            // Gemini (from gemini-cli)
            if google_oauth::has_gemini_cli_credentials().await {
                println!("  ✅ Gemini: Logged in (via gemini-cli)");
            } else {
                println!("  ❌ Gemini: Not logged in. Run `gemini` CLI to authenticate.");
            }

            // Qwen
            if qwen_oauth::has_qwen_credentials().await {
                println!("  ✅ Qwen: Logged in");
            } else {
                println!("  ❌ Qwen: Not logged in. Run `gestalt_cli qwen-login`");
            }

            // OpenAI
            if std::env::var("OPENAI_API_KEY").is_ok() {
                println!("  ✅ OpenAI: API key set");
            } else {
                println!("  ❌ OpenAI: OPENAI_API_KEY not set");
            }

            println!("\n  ℹ️  Ollama: Always available (local)");
            return;
        }
        None => {
            // Continue with the ask flow
        }
    }

    // 4. Initialize MCP Service (Experimental)
    info!("Initializing MCP Service...");
    let mut mcp_service = McpService::new();
    if let Err(e) = mcp_service.initialize_all().await {
        tracing::warn!("Failed to initialize MCP service: {}", e);
    } else {
        let count = mcp_service.get_clients().len();
        if count > 0 {
            info!("✅ Initialized {} MCP servers.", count);
            let tools = mcp_service.list_tools().await;
            info!("🛠️  Available MCP Tools: {:?}", tools);
        } else {
            info!("No MCP servers found in configuration.");
        }
    }

    // Load Config
    let config = AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {}. Using defaults.", e);
        AppConfig::default()
    });

    // 5. Initialize Providers
    let mut providers: Vec<(String, Arc<dyn LlmProvider>)> = Vec::new();

    // Gemini
    let gemini_model = if cli.gemini_model != "gemini-2.0-flash" {
        cli.gemini_model.clone()
    } else {
        config.gemini.model.clone()
    };

    if google_oauth::has_gemini_cli_credentials().await {
        info!("Using Gemini with gemini-cli OAuth credentials.");
        let gemini = GeminiOAuthProvider::new(gemini_model);
        providers.push(("Gemini".to_string(), Arc::new(gemini)));
    } else if let Some(key) = config
        .gemini
        .api_key
        .clone()
        .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
    {
        std::env::set_var("GOOGLE_API_KEY", key);
        let gemini = GeminiProvider::new(gemini_model);
        providers.push(("Gemini (API Key)".to_string(), Arc::new(gemini)));
    } else {
        info!("No Gemini credentials found.");
    }

    // Qwen
    let qwen_model = if cli.qwen_model != "qwen-coder" {
        cli.qwen_model.clone()
    } else {
        config.qwen.model.clone()
    };
    if qwen_oauth::has_qwen_credentials().await {
        info!("Using Qwen with OAuth credentials.");
        let qwen = QwenProvider::new(qwen_model);
        providers.push(("Qwen".to_string(), Arc::new(qwen)));
    }

    // OpenAI
    let openai_model = if cli.openai_model != "gpt-4" {
        cli.openai_model.clone()
    } else {
        config.openai.model.clone()
    };
    if let Some(key) = config
        .openai
        .api_key
        .clone()
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
    {
        std::env::set_var("OPENAI_API_KEY", key);
        let openai = OpenAIProvider::new(openai_model);
        providers.push(("OpenAI".to_string(), Arc::new(openai)));
    }

    // Ollama
    let ollama_model = if cli.ollama_model != "llama2" {
        cli.ollama_model.clone()
    } else {
        config.ollama.model.clone()
    };
    let ollama_url = if cli.ollama_url != "http://localhost:11434" {
        cli.ollama_url.clone()
    } else {
        config.ollama.base_url.clone()
    };
    let ollama = OllamaProvider::new(ollama_url, ollama_model);
    providers.push(("Ollama".to_string(), Arc::new(ollama)));

    if providers.is_empty() {
        eprintln!("No LLM providers available. Run `gestalt_cli status` to see auth status.");
        return;
    }

    // Check if prompt is provided
    if let Some(prompt) = &cli.prompt {
        // Single Shot Mode
        process_prompt(prompt, &providers, &cli, cli.consensus).await;
    } else {
        // REPL Mode
        println!("🤖 Gestalt REPL (v0.1.0)");
        println!("Type '/exit' to quit, '/clear' to clear history.");

        let mut rl = DefaultEditor::new().unwrap();
        if rl.load_history("history.txt").is_err() {
            println!("No previous history.");
        }

        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    rl.add_history_entry(line).unwrap();

                    if line == "/exit" {
                        break;
                    }
                    if line == "/clear" {
                        rl.clear_history().unwrap();
                        println!("History cleared.");
                        continue;
                    }
                    if line == "/config" {
                        println!("{:#?}", config);
                        continue;
                    }

                    process_prompt(line, &providers, &cli, cli.consensus).await;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL-C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL-D");
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
        rl.save_history("history.txt").unwrap();
    }
}

async fn process_prompt(
    prompt: &str,
    providers: &Vec<(String, Arc<dyn LlmProvider>)>,
    cli: &Cli,
    consensus: bool,
) {
    // --- Context Engine ---
    let mut final_prompt = prompt.to_string();
    if cli.context {
        info!("🧠 Context Engine: Analyzing project...");
        let root = Path::new(".");
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

        final_prompt = format!("CONTEXT:\n{}\n\nUSER PROMPT:\n{}", context_str, prompt);
        info!(
            "🧠 Context Engine: Added {} chars of context.",
            context_str.len()
        );
    }

    if consensus {
        info!("Starting Gestalt Agent Consensus...");
        let service = ConsensusService::new(providers.clone());
        let result = service.ask_all(&final_prompt).await;

        match serde_json::to_string_pretty(&result) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Failed to serialize result: {}", e),
        }
    } else {
        // Pick the first available provider
        if let Some((name, provider)) = providers.first() {
            info!("Using primary provider: {}", name);
            let request = LlmRequest {
                prompt: final_prompt,
                model: String::new(),
                temperature: 0.7,
                max_tokens: None,
            };

            // Try streaming first
            match provider.stream(request.clone()).await {
                Ok(mut stream) => {
                    print!("🤖 ");
                    std::io::stdout().flush().unwrap();
                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                print!("{}", chunk);
                                std::io::stdout().flush().unwrap();
                            }
                            Err(e) => {
                                eprintln!("\nStream Error: {}", e);
                                break;
                            }
                        }
                    }
                    println!(); // Newline at end
                }
                Err(_) => {
                    // Fallback to generate if stream not implemented
                    match provider.generate(request).await {
                        Ok(response) => println!("{}", response.content),
                        Err(e) => eprintln!("Error from {}: {}", name, e),
                    }
                }
            }
        }
    }
}
