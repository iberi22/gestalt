//! Gestalt standalone MCP server
//!
//! Exposes Gestalt tools via the Model Context Protocol (MCP).
//! Supports both HTTP/SSE and stdio transports.

mod app_context;
mod gestalt_tools;
mod tools;

use std::sync::Arc;

use app_context::GestaltAppContext;
use clap::{Parser, Subcommand};
use mcp_protocol_sdk::server::McpServer;
use mcp_protocol_sdk::server::HttpMcpServer;
use mcp_protocol_sdk::transport::{HttpServerTransport, StdioServerTransport};
use tracing::info;

/// Gestalt MCP Server — exposes Gestalt tools via MCP
#[derive(Parser, Debug)]
#[command(name = "gestalt_mcp", about = "Gestalt MCP Server")]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Transport mode: "http" (SSE) or "stdio"
    #[arg(long, default_value = "http")]
    transport: String,

    /// Bind address (HTTP mode only)
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind: String,

    /// Server name advertised to clients
    #[arg(long, default_value = "gestalt-mcp")]
    name: String,

    /// Server version advertised to clients
    #[arg(long, default_value = "1.0.0")]
    version: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List registered tools and exit
    ListTools,
    /// Check server health
    Health,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .init();

    // Create shared app context (holds state that tools may access).
    let ctx = Arc::new(GestaltAppContext::new());

    // Handle non-server commands
    if let Some(cmd) = &args.command {
        match cmd {
            Commands::ListTools => {
                let server = build_server(args.name.clone(), args.version.clone());
                register_all_tools(&server, ctx.clone()).await?;
                let tools = server.list_tools().await?;
                println!("📋 Registered tools ({}):", tools.len());
                for tool in &tools {
                    println!(
                        "  • {}{}",
                        tool.name,
                        tool.description
                            .as_ref()
                            .map(|d| format!(": {}", d))
                            .unwrap_or_default()
                    );
                }
                return Ok(());
            }
            Commands::Health => {
                let status = ctx.status();
                println!("✅ Gestalt MCP Server ({} v{})", args.name, args.version);
                println!("{}", serde_json::to_string_pretty(&status).unwrap());
                return Ok(());
            }
        }
    }

    info!(
        "Starting Gestalt MCP Server ({} v{}) — {} transport",
        args.name, args.version, args.transport
    );
    println!(
        "🚀 Gestalt MCP Server ({} v{})",
        args.name, args.version
    );
    println!("   Transport: {}", args.transport);

    match args.transport.as_str() {
        "http" => {
            println!("   Bind: {}", args.bind);

            // Create HttpMcpServer — handles HTTP transport wiring automatically
            let mut http_server = HttpMcpServer::new(args.name.clone(), args.version.clone());
            let server_handle = http_server.server().await;

            // Register tools on the underlying server
            {
                let guard = server_handle.lock().await;
                register_all_tools(&guard, ctx.clone()).await?;
            }

            // Create HTTP transport and start
            let transport = HttpServerTransport::new(&args.bind);
            http_server.start(transport).await?;

            info!("Server started, listening on http://{}", args.bind);
            println!("   Listening on: http://{}", args.bind);

            // Keep process alive until Ctrl+C
            tokio::signal::ctrl_c().await?;
            info!("Shutting down on Ctrl+C");
            println!("\nShutting down...");
            http_server.stop().await?;
        }
        "stdio" => {
            println!("   Mode: stdio (read from stdin, write to stdout)");

            // Create base McpServer for stdio transport
            let mut server = build_server(args.name.clone(), args.version.clone());
            register_all_tools(&server, ctx.clone()).await?;

            let transport = StdioServerTransport::new();
            server.start(transport).await?;
        }
        other => {
            anyhow::bail!("Unsupported transport: {}. Use 'http' or 'stdio'.", other);
        }
    }

    Ok(())
}

/// Create a bare McpServer (tools are added via register_all_tools).
fn build_server(name: String, version: String) -> McpServer {
    McpServer::new(name, version)
}

/// Register all tools — both the built-in ones (tools.rs) and the
/// Gestalt-specific ones (gestalt_tools.rs).
async fn register_all_tools(
    server: &McpServer,
    ctx: Arc<GestaltAppContext>,
) -> anyhow::Result<()> {
    // 1. Built-in tools (file, git, shell, task, echo, etc.)
    tools::register_standard_tools(server).await?;

    // 2. Gestalt-specific tools (belief graph, search, agent, etc.)
    gestalt_tools::register_gestalt_tools(server, ctx).await?;

    Ok(())
}
