use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "swarm")]
#[command(about = "Gestalt Swarm CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check the status of the swarm
    Status,
    /// Run a swarm task
    Run {
        /// The goal for the swarm
        #[arg(short, long)]
        goal: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)))
        .init();

    match args.command {
        Some(Commands::Status) => {
            println!("🐝 Gestalt Swarm is active and ready.");
        }
        Some(Commands::Run { goal }) => {
            info!("Running swarm task: {}", goal);
            println!("🚀 Executing goal: {}", goal);
            // Swarm orchestration logic would go here
        }
        None => {
            println!("Gestalt Swarm CLI. Use --help for available commands.");
        }
    }

    Ok(())
}
