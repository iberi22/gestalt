use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::fs;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiSettings {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

pub async fn load_gemini_mcp_configs() -> anyhow::Result<HashMap<String, McpServerConfig>> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let settings_path = home.join(".gemini").join("settings.json");

    if !settings_path.exists() {
        info!("No gemini settings found at {:?}", settings_path);
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&settings_path).await?;
    let settings: GeminiSettings = serde_json::from_str(&content)?;

    info!(
        "Loaded {} MCP servers from Gemini settings",
        settings.mcp_servers.len()
    );
    Ok(settings.mcp_servers)
}
