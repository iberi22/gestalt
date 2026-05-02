# Gestalt — Agent Developer Guide

## Project Overview

Gestalt is a **multi-agent orchestration framework** written in Rust. It provides:
- A REPL/CLI interface (`gestalt_cli`)
- Core agent framework (`gestalt_core`)
- Timeline tracking (`gestalt_timeline`)
- Swarm capabilities (`gestalt_swarm`)

## Key Packages

| Package | Purpose | Entry Point |
|---|---|---|
| `gestalt_cli` | CLI binary & REPL | `gestalt_cli/src/main.rs` |
| `gestalt_core` | Core agent tools, tools, context | `gestalt_core/src/` |
| `gestalt_timeline` | Event timeline tracking | `gestalt_timeline/src/` |
| `gestalt_swarm` | Multi-agent swarm logic | `gestalt_swarm/src/` |
| `synapse-agentic` | Agentic primitives | `synapse-agentic/src/` |

## Building

```bash
# Full project (all crates)
cargo build --release

# CLI only (most common)
cargo build --release -p gestalt_cli

# Run the CLI
cargo run --release -p gestalt_cli -- serve --host 0.0.0.0 --port 10000
```

## Running

```bash
# REPL mode
cargo run --release -p gestalt_cli

# Serve mode (HTTP API)
cargo run --release -p gestalt_cli -- serve --host 0.0.0.0 --port 10000
```

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `DEEPSEEK_API_KEY` | Yes | DeepSeek API key for LLM calls |
| `RUST_LOG` | No | Log level, e.g. `info` or `debug` |

## Common Tasks

### Run Tests
```bash
cargo test -p gestalt_core --all-targets
cargo test -p gestalt_cli --all-targets
cargo test -p gestalt_timeline --lib
```

### Format & Lint
```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

### Check for Security Vulnerabilities
```bash
cargo audit
```

## Build Errors

If you see `async fn is not permitted in Rust 2015`, the crate is using Rust 2021 edition. Make sure your toolchain supports 2021:
```bash
rustup update stable
```

## Render Deployment

The `render.yaml` file defines the Render blueprint. Build/start commands for the service:

```
Build: cargo build --release -p gestalt_cli
Start: ./target/release/gestalt_cli serve --host 0.0.0.0 --port 10000
```

Required environment variable: `DEEPSEEK_API_KEY`
