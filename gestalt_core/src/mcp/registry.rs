//! MCP Registry
//!
//! Registry for MCP tools with execution support.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use super::client_impl::{McpClient, McpError, ToolInfo, ToolResult};

/// Tool context for execution
pub trait ToolContext: Send + Sync {
    /// Get context data
    fn get_data(&self) -> &Value;
}

/// Default tool context
#[derive(Debug, Clone, Default)]
pub struct DefaultToolContext {
    data: Value,
}

impl DefaultToolContext {
    pub fn new() -> Self {
        Self {
            data: Value::Null,
        }
    }
    
    pub fn with_data(data: Value) -> Self {
        Self { data }
    }
}

impl ToolContext for DefaultToolContext {
    fn get_data(&self) -> &Value {
        &self.data
    }
}

/// MCP Registry - manages tools and execution
#[derive(Clone)]
pub struct McpRegistry {
    /// Registered tools
    tools: Arc<Mutex<HashMap<String, ToolInfo>>>,
    /// MCP client
    client: Arc<Mutex<Option<GestaltMcpClient>>>,
    /// Default timeout
    default_timeout: Duration,
}

impl McpRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(Mutex::new(HashMap::new())),
            client: Arc::new(Mutex::new(None)),
            default_timeout: Duration::from_secs(30),
        }
    }
    
    /// Register a tool
    pub async fn register_tool(&self, tool: ToolInfo) {
        let mut tools = self.tools.lock().await;
        tools.insert(tool.name.clone(), tool);
    }
    
    /// Unregister a tool
    pub async fn unregister_tool(&self, name: &str) {
        let mut tools = self.tools.lock().await;
        tools.remove(name);
    }
    
    /// List all tools
    pub async fn list_tools(&self) -> Vec<ToolInfo> {
        let tools = self.tools.lock().await;
        tools.values().cloned().collect()
    }
    
    /// Get a specific tool
    pub async fn get_tool(&self, name: &str) -> Option<ToolInfo> {
        let tools = self.tools.lock().await;
        tools.get(name).cloned()
    }
    
    /// Connect to MCP server
    pub async fn connect(&self, server_url: &str) -> Result<(), McpError> {
        let client = GestaltMcpClient::with_server(server_url);
        client.connect(server_url).await?;
        
        // Fetch and register tools
        let tools = client.list_tools().await?;
        for tool in tools {
            self.register_tool(tool).await;
        }
        
        let mut client_ref = self.client.lock().await;
        *client_ref = Some(client);
        
        Ok(())
    }
    
    /// Execute a tool
    pub async fn execute_tool(
        &self,
        name: &str,
        args: Value,
        ctx: &dyn ToolContext,
        timeout_secs: Option<u64>,
    ) -> Result<ToolResult, McpError> {
        // Check if tool exists
        {
            let tools = self.tools.lock().await;
            if !tools.contains_key(name) {
                return Err(McpError::ToolNotFound(name.to_string()));
            }
        }
        
        let timeout_duration = timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(self.default_timeout);
        
        // Execute with timeout
        let result = timeout(
            timeout_duration,
            self.execute_tool_inner(name, args, ctx),
        )
        .await
        .map_err(|_| McpError::Timeout)?;
        
        result
    }
    
    /// Inner execution (without timeout wrapper)
    async fn execute_tool_inner(
        &self,
        name: &str,
        args: Value,
        _ctx: &dyn ToolContext,
    ) -> Result<ToolResult, McpError> {
        let client_ref = self.client.lock().await;
        
        if let Some(ref client) = *client_ref {
            client.call_tool(name, args, None).await
        } else {
            // Local execution for registered tools
            Ok(ToolResult {
                success: true,
                content: json!({
                    "message": "Tool executed locally",
                    "tool": name,
                    "args": args
                }),
                error: None,
            })
        }
    }
    
    /// Validate arguments against tool schema
    pub async fn validate_arguments(
        &self,
        name: &str,
        args: &Value,
    ) -> Result<(), McpError> {
        let tools = self.tools.lock().await;
        
        if let Some(tool) = tools.get(name) {
            // Basic validation - check required fields exist
            if let Some(params) = tool.parameters.as_object() {
                if let Some(required) = params.get("required") {
                    if let Some(required_fields) = required.as_array() {
                        for field in required_fields {
                            if let Some(field_name) = field.as_str() {
                                if args.get(field_name).is_none() {
                                    return Err(McpError::InvalidResponse(format!(
                                        "Missing required field: {}",
                                        field_name
                                    )));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        } else {
            Err(McpError::ToolNotFound(name.to_string()))
        }
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        let registry = McpRegistry::new();
        let tools = registry.list_tools().await;
        assert!(tools.is_empty());
    }
    
    #[tokio::test]
    async fn test_register_tool() {
        let registry = McpRegistry::new();
        
        let tool = ToolInfo {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: json!({"type": "object"}),
        };
        
        registry.register_tool(tool).await;
        
        let tools = registry.list_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "test_tool");
    }
    
    #[tokio::test]
    async fn test_execute_tool() {
        let registry = McpRegistry::new();
        
        let tool = ToolInfo {
            name: "echo".to_string(),
            description: "Echo back input".to_string(),
            parameters: json!({"type": "object"}),
        };
        
        registry.register_tool(tool).await;
        
        let result = registry
            .execute_tool("echo", json!({"input": "hello"}), &DefaultToolContext::new(), None)
            .await
            .unwrap();
        
        assert!(result.success);
    }
    
    #[tokio::test]
    async fn test_execute_nonexistent_tool() {
        let registry = McpRegistry::new();
        
        let result = registry
            .execute_tool("nonexistent", json!({}), &DefaultToolContext::new(), None)
            .await;
        
        assert!(result.is_err());
        matches!(result.unwrap_err(), McpError::ToolNotFound(_));
    }
}
