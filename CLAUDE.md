# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Gestalt is a context-aware AI assistant for the terminal that intelligently gathers project context (files, structure, configs) to give LLMs the full picture of your work. It's a Rust workspace with multiple crates.

## Key Commands

### Building
```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build -p gestalt_cli
cargo build -p gestalt_core
cargo build -p gestalt_timeline

# Run the CLI
cargo run -p gestalt_cli -- --help

# Run the timeline server
cargo run -p gestalt_timeline --bin gestalt -- server --port 3000
```

### Linting & Formatting
```bash
# Format code
cargo fmt

# Run clippy
cargo clippy
cargo clippy -p gestalt_core --fix  # with auto-fix
```

### Testing
```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p gestalt_core
cargo test -p gestalt_timeline

# Run a specific test
cargo test -p gestalt_timeline test_name

# Run e2e tests
cargo test -p gestalt_timeline --test e2e_runtime

# Run integration tests
cargo test -p gestalt_timeline --test integration
```

### Development
```bash
# Initialize config
gestalt config init

# Start MCP server
gestalt serve --port 3000
```

## Architecture

This is a Rust workspace with the following main crates:

| Crate | Purpose |
|-------|---------|
| `gestalt_core` | Generic logic, domain models, ports/adapters, context engine |
| `gestalt_cli` | Command-line interface using Clap and Rustyline |
| `gestalt_timeline` | Tokio + SurrealDB orchestration, agent runtime |
| `gestalt_mcp` | Model Context Protocol server |
| `synapse-agentic` | Hive actor model for agent supervision |

### Key Design Patterns

1. **Ports & Adapters (Hexagonal)**: `gestalt_core` uses ports in `src/ports/` (inbound/outbound) with implementations in `src/adapters/`

2. **Async Non-Blocking**: All long-running agent actions spawn `tokio::task` and return immediately with a `JobId`

3. **Timeline-First**: Every action, command, result, or state change is recorded in a universal timeline with timestamps

4. **Context Compaction**: Older reasoning steps are automatically summarized using tiktoken for estimation to manage context windows

5. **Virtual File System (VFS)**: Agents operate in a volatile memory-mapped workspace before committing to disk

### Data Flow
```
CLI -> Synapse Hive -> AgentRuntime -> LLM/_tools/VFS
                    -> JobManager (async tasks)
                    -> Timeline (SurrealDB)
```

## Configuration

Configuration uses TOML files with environment variable overrides. See `gestalt_core/src/application/CONFIG.md` for details.

- Config file locations: `./gestalt.toml`, `./config/gestalt.toml`, `~/.config/gestalt/gestalt.toml`
- Env vars use prefix `GESTALT_` with double underscores: `GESTALT_LLM__DEFAULT_PROVIDER=openai`

## Important Files

- `.gitcore/ARCHITECTURE.md` - Detailed system design decisions
- `.github/copilot-instructions.md` - Git-Core Protocol workflow
- `RULES.md` - Development rules for AI agents
- `gestalt_core/src/application/CONFIG.md` - Configuration system

## Git Workflow

Follow the Git-Core Protocol:
1. Create issues in `.github/issues/` directory
2. Use `gh issue comment` for updates
3. Commit format: `type(scope): description #issue`
4. Run `cargo fmt` and `cargo clippy` before commits
