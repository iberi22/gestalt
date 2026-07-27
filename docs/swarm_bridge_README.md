# Gestalt Swarm — Parallel Agent Execution Bridge

Gestalt Swarm is a parallel execution bridge designed to run multiple CLI-based agents concurrently using Python's `asyncio`. It is primarily used as a tool for OpenClaw to perform comprehensive codebase analysis, dependency checks, and security audits in a fraction of the time compared to sequential execution.

## 🚀 Quick Start

### Basic Usage
Run a swarm with a specific goal. The bridge will automatically select the most relevant agents based on your goal description.

```bash
python swarm_bridge.py --goal "analyze the codebase for security issues"
```

### Manual Agent Selection
You can override the automatic selection by providing a comma-separated list of agent IDs.

```bash
python swarm_bridge.py --goal "check git status" --agents "git_status,git_analyzer"
```

### JSON Output
For machine-to-machine communication (e.g., when called by another agent), use the `--json` flag.

```bash
python swarm_bridge.py --goal "analyze deps" --agents "dep_check" --json
```

### Streaming / Watch Mode
Enable `--watch` to poll for results as each agent completes. This is useful for long-running tasks.

```bash
python swarm_bridge.py --goal "comprehensive audit" --watch --poll-interval 500
```

## 📋 Configuration

You can configure default limits via environment variables:

- `GESTALT_MAX_AGENTS`: Maximum number of parallel agents (default: 10)
- `GESTALT_RATE_LIMIT`: API rate limit per minute, used to calculate optimal parallelism (default: 100)

## 🐝 Available Agents

| Agent ID | Description |
|----------|-------------|
| `code_analyzer` | Counts Rust files and lines |
| `dep_check` | Lists top-level dependencies |
| `cargo_check` | Runs `cargo check` |
| `git_analyzer` | Shows recent git commits |
| `git_status` | Shows current git status |
| `file_scanner` | Lists all files in the project |
| `security_audit` | Scans for TODOs, FIXMEs, and unsafe code |
| `metrics` | Detailed dependency tree |
| `doc_gen` | Lists all Markdown files |
| `api_tester` | Checks health of local API |
| `find_todos` | Locates TODOs with line numbers |
| `rust_files` | Lists all `.rs` files |
| `env_check` | Checks `.env.example` |

## 🛠️ Integration

Gestalt Swarm is registered as an OpenClaw skill. When using OpenClaw, you can invoke it via:

```bash
exec: python swarm_bridge.py --goal "..." --json
```
