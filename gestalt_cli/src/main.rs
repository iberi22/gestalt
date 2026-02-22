use clap::{Parser, Subcommand};
use dotenv::dotenv;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::Write;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use synapse_agentic::prelude::{DecisionContext, DecisionEngine, GeminiProvider, MinimaxProvider};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Prompt single-shot. If omitted, starts REPL mode.
    #[arg(short, long)]
    prompt: Option<String>,

    /// Add local context hints to the prompt.
    #[arg(long, default_value_t = true)]
    context: bool,

    /// Provider: gemini | minimax.
    #[arg(long, default_value = "gemini")]
    provider: String,

    /// Model ID for selected provider.
    #[arg(long, default_value = "gemini-2.0-flash")]
    model: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show runtime/provider status.
    Status,
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    dotenv().ok();
    let cli = Cli::parse();

    if let Some(Commands::Status) = cli.command {
        print_status(&cli);
        return;
    }

    let engine = match init_decision_engine(&cli).await {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("❌ {}", err);
            return;
        }
    };

    if let Some(prompt) = &cli.prompt {
        if let Err(err) = handle_prompt(prompt, &cli, engine.as_ref()).await {
            eprintln!("❌ {}", err);
        }
    } else if let Err(err) = run_repl(&cli, engine).await {
        eprintln!("❌ {}", err);
    }
}

fn print_status(cli: &Cli) {
    println!("🔧 Gestalt CLI Status");
    println!("provider: {}", cli.provider);
    println!("model: {}", cli.model);
    println!("context: {}", cli.context);
    println!();
    println!("env:");
    println!(
        "  GEMINI_API_KEY: {}",
        if std::env::var("GEMINI_API_KEY").is_ok() || std::env::var("GOOGLE_API_KEY").is_ok() {
            "set"
        } else {
            "missing"
        }
    );
    println!(
        "  MINIMAX_API_KEY: {}",
        if std::env::var("MINIMAX_API_KEY").is_ok() {
            "set"
        } else {
            "missing"
        }
    );
}

async fn init_decision_engine(cli: &Cli) -> Result<Arc<DecisionEngine>, String> {
    let provider_name = cli.provider.trim().to_lowercase();
    let mut builder = DecisionEngine::builder();

    match provider_name.as_str() {
        "gemini" => {
            let api_key = std::env::var("GEMINI_API_KEY")
                .or_else(|_| std::env::var("GOOGLE_API_KEY"))
                .map_err(|_| {
                    "GEMINI_API_KEY/GOOGLE_API_KEY not found. Set one to use --provider gemini."
                        .to_string()
                })?;
            builder = builder.with_provider(GeminiProvider::new(api_key, cli.model.clone()));
        }
        "minimax" => {
            let api_key = std::env::var("MINIMAX_API_KEY").map_err(|_| {
                "MINIMAX_API_KEY not found. Set it to use --provider minimax.".to_string()
            })?;
            let group_id = std::env::var("MINIMAX_GROUP_ID").unwrap_or_default();
            builder =
                builder.with_provider(MinimaxProvider::new(api_key, group_id, cli.model.clone()));
        }
        other => {
            return Err(format!(
                "Unsupported provider '{}'. Use gemini or minimax.",
                other
            ));
        }
    }

    Ok(Arc::new(builder.build()))
}

fn enrich_prompt(input: &str, use_context: bool, history: &[String]) -> String {
    if !use_context && history.is_empty() {
        return input.to_string();
    }

    let mut prompt = String::new();
    if use_context {
        prompt.push_str("MODE: interactive_cli\n");
    }
    if !history.is_empty() {
        prompt.push_str("HISTORY:\n");
        prompt.push_str(&history.join("\n"));
        prompt.push('\n');
    }
    prompt.push_str("USER:\n");
    prompt.push_str(input);
    prompt
}

async fn stream_text(text: &str) {
    print!("🤖 ");
    let _ = std::io::stdout().flush();

    for token in text.split_whitespace() {
        print!("{} ", token);
        let _ = std::io::stdout().flush();
        sleep(Duration::from_millis(18)).await;
    }
    println!();
}

async fn handle_prompt(prompt: &str, cli: &Cli, engine: &DecisionEngine) -> Result<(), String> {
    let full = enrich_prompt(prompt, cli.context, &[]);
    info!("Processing single-shot prompt");
    let decision = engine
        .decide(&DecisionContext::new("cli").with_summary(&full))
        .await
        .map_err(|e| format!("Decision engine error: {}", e))?;
    stream_text(&decision.reasoning).await;
    Ok(())
}

async fn run_repl(cli: &Cli, engine: Arc<DecisionEngine>) -> Result<(), String> {
    println!("🤖 Gestalt REPL");
    println!("Commands: /exit, /clear, /config");

    let mut rl = DefaultEditor::new().map_err(|e| format!("Readline init failed: {}", e))?;
    let _ = rl.load_history("history.txt");
    let mut history: Vec<String> = Vec::new();

    loop {
        match rl.readline(">> ") {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(input);

                if input == "/exit" {
                    break;
                }
                if input == "/clear" {
                    history.clear();
                    let _ = rl.clear_history();
                    println!("History cleared.");
                    continue;
                }
                if input == "/config" {
                    println!(
                        "provider={} model={} context={}",
                        cli.provider, cli.model, cli.context
                    );
                    continue;
                }

                let full = enrich_prompt(input, cli.context, &history);
                let decision = engine
                    .decide(&DecisionContext::new("repl").with_summary(&full))
                    .await
                    .map_err(|e| format!("Decision engine error: {}", e))?;

                stream_text(&decision.reasoning).await;
                history.push(format!("user: {}", input));
                history.push(format!("assistant: {}", decision.reasoning));
            }
            Err(ReadlineError::Interrupted) => {
                println!();
                break;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(err) => {
                warn!("Readline error: {:?}", err);
                return Err(format!("Readline error: {:?}", err));
            }
        }
    }

    let _ = rl.save_history("history.txt");
    Ok(())
}
