use gestalt_mcp::{start_server, start_stdio_server};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.contains(&"--http".to_string()) {
        println!("🚀 Starting HTTP server on http://127.0.0.1:3000");
        start_server().await?;
    } else {
        println!("🔌 Starting Stdio server...");
        start_stdio_server().await?;
    }
    
    Ok(())
}
