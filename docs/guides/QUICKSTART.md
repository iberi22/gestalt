# 🚀 Quick Start Guide

## Prerequisites

- Rust 1.70+ (with Cargo)
- Git
- SurrealDB (embedded or Docker)

## Installation

```bash
# Clone the repository
git clone https://github.com/iberi22/gestalt-rust.git
cd gestalt-rust

# Build the project
cargo build --release

# Or build specific components
cargo build -p gestalt_timeline
cargo build -p gestalt_cli
```

## API Key Setup

Gestalt uses LLM providers for AI capabilities. Configure API keys via environment variables:

### Gemini (Google)

```bash
export GEMINI_API_KEY="your-google-ai-studio-api-key"
```

Get your key at: https://aistudio.google.com/app/apikey

### MiniMax

```bash
export MINIMAX_API_KEY="your-minimax-api-key"
export MINIMAX_GROUP_ID="your-group-id"  # Optional
```

Get your key at: https://platform.minimax.chat/

### Verification

```bash
# Test that API keys are recognized
cargo run -p gestalt_timeline -- --prompt "Hello" --model gemini-2.0-flash
```

You should see `🚀 Initializing Gemini resilient provider...` in the logs.

## Database Setup

### Option 1: Embedded (Simplest)

SurrealDB runs embedded - no additional setup needed.

### Option 2: Docker

```bash
docker run -p 8000:8000 surrealdb/surrealdb:latest start
```

Set environment:
```bash
export SURREAL_URL="ws://localhost:8000"
export SURREAL_USER="root"
export SURREAL_PASS="root"
```

## Basic Usage

### CLI Mode

```bash
# Start the timeline orchestrator
cargo run -p gestalt_timeline --bin gestalt

# Interactive commands
gestalt add-project my-app
gestalt add-task my-app "Implement feature X"
gestalt list-projects
gestalt timeline --since=1h
```

### Agent Mode

```bash
# Run with AI decision engine
cargo run -p gestalt_timeline --bin gestalt -- \
  --prompt "Review the codebase and suggest improvements" \
  --context
```

### Configuration

Edit `config/gestalt.toml`:

```toml
[agent]
id = "gestalt-01"

[cognition]
provider = "gemini"  # or "minimax", "auto"
model_id = "gemini-2.0-flash"

[database]
url = "mem"  # or "ws://localhost:8000"
```

## MCP Server

Start the MCP server for external tool integration:

```bash
cargo run -p gestalt_mcp
# Server runs on http://127.0.0.1:3000
```

Available tools: echo, analyze_project, list_files, read_file, get_context, search_code, exec_command, git_status, git_log, file_tree, grep, create_file, web_fetch, system_info, task_create, task_status, task_list

## Troubleshooting

### "No external LLM providers configured"

Ensure API keys are set:
```bash
echo $GEMINI_API_KEY  # Should print your key
```

### Build errors

```bash
# Clean and rebuild
cargo clean
cargo check --workspace
```

### Database connection issues

Verify SurrealDB is running:
```bash
curl http://localhost:8000/health
```
