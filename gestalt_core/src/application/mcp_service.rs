use crate::adapters::mcp::client_impl::McpClientAdapter;
use crate::adapters::mcp::config::{load_gemini_mcp_configs, McpServerConfig};
use crate::ports::outbound::mcp_client::McpClientPort;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

pub struct McpService {
    clients: HashMap<String, Arc<dyn McpClientPort>>,
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}

impl McpService {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub async fn initialize_all(&mut self) -> anyhow::Result<()> {
        let configs = load_gemini_mcp_configs().await?;

        for (name, config) in configs {
            info!("Initializing MCP server: {}", name);
            if let Err(e) = self.connect_server(&name, &config).await {
                error!("Failed to connect to MCP server {}: {}", name, e);
            }
        }

        Ok(())
    }

    async fn connect_server(&mut self, name: &str, config: &McpServerConfig) -> anyhow::Result<()> {
        let adapter = McpClientAdapter::new(config.command.clone(), config.args.clone()).await?;

        info!("Successfully initialized MCP server: {}", name);
        self.clients.insert(name.to_string(), Arc::new(adapter));

        Ok(())
    }

    pub fn get_clients(&self) -> &HashMap<String, Arc<dyn McpClientPort>> {
        &self.clients
    }

    pub async fn list_tools(&self) -> Vec<String> {
        let mut tools = Vec::new();
        for (name, client) in &self.clients {
            match client.list_tools().await {
                Ok(tools_list) => {
                    for tool in tools_list {
                        tools.push(format!("{}::{}", name, tool.name));
                    }
                }
                Err(e) => {
                    error!("Failed to list tools for {}: {}", name, e);
                }
            }
        }
        tools
    }
}
