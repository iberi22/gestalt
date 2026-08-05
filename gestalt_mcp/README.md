# gestalt_mcp — Gestalt Standalone MCP Server

Exposes Gestalt multi-agent orchestration capabilities via the **Model Context Protocol (MCP)**.

## Features

- **HTTP/SSE transport** — MCP-over-HTTP with Server-Sent Events
- **Stdio transport** — MCP-over-stdin/stdout for embedding in AI toolchains
- **18+ built-in tools**:
  - **Standard**: `echo`, `analyze_project`, `search_code`, `git_status`, `git_log`, `file_read`, `file_tree`, `system_info`, `shell_execute`, `task_create`, `task_list`, `task_status`
  - **Gestalt**: `server_status`, `gestalt_belief_query`, `gestalt_search`, `gestalt_project_analyze`, `gestalt_registry_info`, `gestalt_agent_run`
- **Shared app context** for stateful tool execution
- **Pluggable architecture** — easily add new MCP tools

## Usage

```bash
# HTTP/SSE mode (default, bind 127.0.0.1:3000)
cargo run -p gestalt_mcp

# Stdio mode
cargo run -p gestalt_mcp -- --transport stdio

# Custom bind address
cargo run -p gestalt_mcp -- --bind 0.0.0.0:8080

# List registered tools
cargo run -p gestalt_mcp -- list-tools

# Show server health
cargo run -p gestalt_mcp -- health

# Verbose logging
cargo run -p gestalt_mcp -- --verbose
```

## Architecture

```
                    ┌─────────────────────────────┐
  MCP Client ─────▶ │     gestalt_mcp server      │
                    │  ┌───────────────────────┐   │
                    │  │  McpServer (SDK)      │   │
                    │  │  ┌─────┐ ┌──────────┐│   │
                    │  │  │tools│ │gestalt_  ││   │
                    │  │  │.rs  │ │tools.rs  ││   │
                    │  │  └─────┘ └──────────┘│   │
                    │  └───────────────────────┘   │
                    │  ┌───────────────────────┐   │
                    │  │  GestaltAppContext     │   │
                    │  │  (shared state)       │   │
                    │  └───────────────────────┘   │
                    │         │                    │
                    │         ▼                    │
                    │  ┌───────────────────────┐   │
                    │  │  gestalt_core         │   │
                    │  │  (beliefs, agents,    │   │
                    │  │   search, config)     │   │
                    │  └───────────────────────┘   │
                    └─────────────────────────────┘
```

## Adding Tools

1. Create a handler struct implementing `ToolHandler` (in `gestalt_tools.rs` or `tools.rs`)
2. Register it in `register_standard_tools()` or `register_gestalt_tools()`
3. For stateful tools, pass `Arc<GestaltAppContext>` to the handler

## API

The server implements the standard MCP protocol:

- `tools/list` — List all registered tools with descriptions and input schemas
- `tools/call` — Execute a tool by name with arguments
- `resources/list` — List available resources
- Server-Sent Events for real-time updates (HTTP transport)

## MCP Client Example

```rust
use mcp_protocol_sdk::client::mcp_client::McpClient;
use mcp_protocol_sdk::transport::http::HttpClientTransport;

let client = McpClient::new("test-client".into(), "1.0.0".into());
let transport = HttpClientTransport::new("http://127.0.0.1:3000");
client.connect(transport).await?;

// List tools
let tools = client.list_tools(None).await?;
println!("Available tools: {}", tools.tools.len());

// Call a tool
let result = client.call_tool("echo".into(), Some([("message".into(), "hello".into())].into())).await?;
```
