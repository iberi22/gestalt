# Quick Start — Gestalt

## Build

```bash
git clone https://github.com/iberi22/gestalt-rust.git
cd gestalt-rust
cargo build --release
```

## Run

```bash
# Main orchestrator
cargo run -p gestalt_timeline --bin gestalt

# Or CLI REPL
cargo run -p gestalt_cli
```

## Configure

```bash
# Required env vars
export GESTALT_DATABASE_URL="surrealdb:memory"  # or file:/tmp/gestalt.db
export GESTALT_LLM__PROVIDER="openai"            # or anthropic
export GESTALT_LLM__OPENAI__API_KEY="sk-..."

# Optional
export GESTALT_LLM__ANTHROPIC__API_KEY="sk-ant-..."
export GESTALT_LOG_LEVEL="info"
```

See [gestalt_core/src/application/CONFIG.md](../gestalt_core/src/application/CONFIG.md) for full config reference.

## Swarm Mode

```bash
# Run gestalt_swarm directly with N agents
cargo run --release -p gestalt_swarm -- --agents 4 --goal "<your task>"
```

## Tools Available

After starting `gestalt`, these tools are registered:

| Tool | Description |
|------|-------------|
| `scan_workspace` | Directory tree of workspace |
| `search_code` | Vector similarity search |
| `execute_shell` | Run shell commands |
| `read_file` | Read file contents |
| `write_file` | Write file contents |
| `git_status` | Git status |
| `git_log` | Git commit log |
| `git_branch` | Git branch operations |
| `git_add` | Git add |
| `git_commit` | Git commit |
| `git_push` | Git push |
| `clone_repo` | Clone repository |
| `list_repos` | List accessible repos |
| `ask_ai` | Query LLM |

## MCP Client

Connect to external MCP servers by adding entries to `config/mcp.json`:

```json
{
  "servers": {
    "my-server": {
      "url": "http://localhost:3000",
      "tools": []
    }
  }
}
```
