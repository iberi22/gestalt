use anyhow::{Context, Result};
use async_trait::async_trait;
use mcp_protocol_sdk::client::mcp_client::McpClient;
use mcp_protocol_sdk::transport::stdio::StdioClientTransport;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ports::outbound::mcp_client::{McpClientPort, ResourceDefinition, ToolDefinition};

pub struct McpClientAdapter {
    client: Arc<McpClient>,
}

impl McpClientAdapter {
    pub async fn new(command: String, args: Vec<String>) -> Result<Self> {
        let transport = StdioClientTransport::new(command, args)
            .await
            .context("Failed to create StdioClientTransport")?;

        let mut client = McpClient::new(
            "gestalt-rust".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        client
            .connect(transport)
            .await
            .context("Failed to connect to MCP server")?;

        Ok(Self {
            client: Arc::new(client),
        })
    }
}

#[async_trait]
impl McpClientPort for McpClientAdapter {
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>> {
        let tools_result = self
            .client
            .list_tools(None)
            .await
            .context("Failed to list tools")?;

        Ok(tools_result
            .tools
            .into_iter()
            .map(|t| ToolDefinition {
                name: t.name,
                description: t.description,
                input_schema: serde_json::to_value(t.input_schema).unwrap_or(Value::Null),
            })
            .collect())
    }

    async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let params: Option<HashMap<String, Value>> = serde_json::from_value(args).ok();
        let result = self
            .client
            .call_tool(name.to_string(), params)
            .await
            .context(format!("Failed to call tool: {}", name))?;

        Ok(serde_json::to_value(result)?)
    }

    async fn list_resources(&self) -> Result<Vec<ResourceDefinition>> {
        let resources_result = self
            .client
            .list_resources(None)
            .await
            .context("Failed to list resources")?;

        Ok(resources_result
            .resources
            .into_iter()
            .map(|r| ResourceDefinition {
                uri: r.uri,
                name: r.name,
                description: r.description,
                mime_type: r.mime_type,
            })
            .collect())
    }

    async fn read_resource(&self, uri: &str) -> Result<String> {
        let resource_result = self
            .client
            .read_resource(uri.to_string())
            .await
            .context(format!("Failed to read resource: {}", uri))?;

        // Assuming the first content is text for simplicity in this port
        if let Some(content) = resource_result.contents.first() {
            match content {
                mcp_protocol_sdk::protocol::types::ResourceContents::Text { text, .. } => {
                    Ok(text.clone())
                }
                mcp_protocol_sdk::protocol::types::ResourceContents::Blob { blob, .. } => {
                    Ok(format!("Binary content (base64): {}", blob))
                }
            }
        } else {
            anyhow::bail!("No content found in resource")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_mcp_client_connectivity() -> Result<()> {
        let mut mock_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // gestalt_core
        mock_path.push("tests");
        mock_path.push("mock_mcp.py");

        let adapter = McpClientAdapter::new(
            "python".to_string(),
            vec![mock_path.to_string_lossy().to_string()],
        )
        .await?;

        let tools = adapter.list_tools().await?;
        assert!(!tools.is_empty());
        assert_eq!(tools[0].name, "echo");

        Ok(())
    }
}
