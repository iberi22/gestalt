# CLAUDE.md — Gestalt Development Guide

You are working on **Gestalt**, a multi-agent orchestration framework in Rust.

## Project Structure

```
gestalt/
├── gestalt_cli/          # CLI binary (entry point)
├── gestalt_core/         # Core library (agents, tools, context)
├── gestalt_timeline/     # Timeline tracking
├── gestalt_swarm/        # Swarm orchestration
├── gestalt_mcp/          # MCP protocol integration
├── gestalt_infra_github/ # GitHub integration
├── synapse-agentic/      # Agentic primitives
└── .github/workflows/    # CI/CD pipelines
```

## Build Commands

| Command | Purpose |
|---|---|
| `cargo build --release -p gestalt_cli` | Build CLI binary |
| `cargo run --release -p gestalt_cli -- serve` | Run server |
| `cargo test -p gestalt_core --all-targets` | Test core |
| `cargo fmt --all` | Format code |
| `cargo clippy --all-targets -- -D warnings` | Lint |

## Render Deployment

Service: `gestalt-sxo4.onrender.com`
Blueprint: `render.yaml`

**Render build settings:**
- Root Directory: leave empty (repo root)
- Build: `cargo build --release -p gestalt_cli`
- Start: `./target/release/gestalt_cli serve --host 0.0.0.0 --port 10000`
- Environment: `DEEPSEEK_API_KEY` (required)

## Error Resolution Workflow

When CI fails:

1. **Check the error** — Most errors are Rust compilation or test failures
2. **Common fixes:**
   - `validate_shell_command` duplicates → check `gestalt_core/src/application/agent/tools.rs`
   - `IndexerError` conversion → check `gestalt_core/src/application/indexer.rs`
   - `async fn` errors → ensure Rust 2021 edition, run `rustup update`
3. **Make the fix** on a branch, push, open PR
4. **CI auto-runs** on every push/PR

## CI Pipeline

See `.github/workflows/ci.yml`:
- Rust format check
- Clippy lint
- Tests for `gestalt_core`, `gestalt_cli`, `gestalt_timeline`
- Security audit with `cargo audit`

## Environment

- Rust toolchain: stable
- Edition: 2021
- Key dependencies: tokio, async-trait, surrealdb, git2
