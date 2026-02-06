# MCP (Model Context Protocol) Integration

This document describes the MCP integration in Gestalt Rust.

## Overview

MCP (Model Context Protocol) is a standardized protocol for connecting AI assistants to external tools and data sources. Gestalt Rust implements MCP client and registry for tool execution.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Gestalt Rust                          │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐  │
│  │   MCP       │───▶│   MCP       │───▶│   Tool     │  │
│  │   Client   │    │  Registry   │    │ Execution  │  │
│  └─────────────┘    └─────────────┘    └────────────┘  │
│         │                                      │       │
│         ▼                                      ▼       │
│  ┌─────────────┐                      ┌────────────┐  │
│  │ MCP Server  │                      │ Local      │  │
│  │ (External)   │                      │ Tools      │  │
│  └─────────────┘                      └────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Usage

### Creating an MCP Client

```rust
use gestalt_core::mcp::{McpClient, GestaltMcpClient};

// Create client
let client = GestaltMcpClient::new();

// Connect to server
client.connect("http://localhost:3000").await?;

// List tools
let tools = client.list_tools().await?;
for tool in tools {
    println!("Tool: {} - {}", tool.name, tool.description);
}

// Call a tool
let result = client
    .call_tool("shell_execute", json!({"command": "ls -la"}), None)
    .await?;
```

### Using the Registry

```rust
use gestalt_core::mcp::{McpRegistry, DefaultToolContext};

let registry = McpRegistry::new();

// Register tools
registry.register_tool(tool_info).await;

// Execute tool
let result = registry
    .execute_tool("my_tool", args, &DefaultToolContext::new(), Some(30))
    .await?;
```

## API Reference

### McpClient Trait

```rust
trait McpClient {
    async fn connect(&self, server_url: &str) -> Result<(), McpError>;
    async fn disconnect(&self) -> Result<(), McpError>;
    async fn list_tools(&self) -> Result<Vec<ToolInfo>, McpError>;
    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        timeout: Option<Duration>,
    ) -> Result<ToolResult, McpError>;
}
```

### McpRegistry

```rust
struct McpRegistry {
    async fn register_tool(&self, tool: ToolInfo);
    async fn unregister_tool(&self, name: &str);
    async fn list_tools(&self) -> Vec<ToolInfo>;
    async fn execute_tool(
        &self,
        name: &str,
        args: Value,
        ctx: &dyn ToolContext,
        timeout_secs: Option<u64>,
    ) -> Result<ToolResult, McpError>;
}
```

## Error Handling

```rust
enum McpError {
    Connection(String),
    ToolNotFound(String),
    Execution(String),
    Timeout,
    InvalidResponse(String),
}
```

## Example: Complete Workflow

```rust
use gestalt_core::mcp::{McpRegistry, GestaltMcpClient, DefaultToolContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = McpRegistry::new();
    
    // Connect to MCP server
    registry.connect("http://localhost:3000").await?;
    
    // Get available tools
    let tools = registry.list_tools().await;
    println!("Available tools: {}", tools.len());
    
    // Execute a tool
    let result = registry
        .execute_tool(
            "file_read",
            json!({"path": "/tmp/test.txt"}),
            &DefaultToolContext::new(),
            None,
        )
        .await?;
    
    println!("Result: {:?}", result);
    Ok(())
}
```

## Configuration

MCP can be configured via TOML:

```toml
[mcp]
enabled = true
server_url = "http://localhost:3000"
default_timeout = 30  # seconds

[[mcp.tools]]
name = "custom_tool"
description = "Custom tool"
```

## Testing

```bash
# Run MCP tests
cargo test -p gestalt_core --lib mcp
```
