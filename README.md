# Gestalt — Multi-Agent Codebase Router

> **Universal AI Agent Orchestration Platform** — CLI-first, Rust-powered.
> Orchestrate, isolate, and integrate multiple AI coding agents against a shared codebase.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-2021+-orange.svg)](https://www.rust-lang.org)

---

## 🚀 Quick Start

### Build
```bash
cargo build --release --workspace
```

### Run Multi-Agent Orchestration
Launch multiple agents in parallel, each in an isolated git worktree. After all agents finish, their changes are integrated via sequential merge-tree.

```bash
cargo run -p gestalt_cli -- run \
  --task "Audit Cargo.toml for outdated dependencies" \
  --agents "cargo audit,cargo outdated" \
  --base-ref main \
  --max-parallel 4 \
  --timeout 300
```

**How it works:**

```
┌─ RunSpec ─────────────────────────────────────┐
│  task: "Audit dependencies"                   │
│  agents: [cargo-audit, cargo-outdated]        │
│  max_parallel: 4, timeout: 300s               │
└──────────────┬────────────────────────────────┘
               │
               ▼
┌─ Router::execute() ───────────────────────────┐
│  1. Resolve base_ref → commit SHA             │
│  2. Create git worktree per agent (isolated)  │
│  3. Spawn agents in parallel (JoinSet)        │
│  4. Run checkpoint per agent (git commit)     │
│  5. Detect file overlaps between agents       │
│  6. Integrate branches (sequential merge)     │
│  7. Cleanup worktrees                         │
│  8. Return RunReport                          │
└───────────────────────────────────────────────┘
```

### REPL Mode
```bash
cargo run -p gestalt_cli -- repl
```

### Serve MCP
```bash
cargo run -p gestalt_cli -- serve --host 0.0.0.0 --port 3000
```

---

## 🧩 Workspace Crates

| Crate | Type | Description |
|-------|------|-------------|
| `gestalt-router` | lib | Multi-agent orchestration: worktree isolation, parallel execution, branch integration, timeline events |
| `gestalt_core` | lib | VFS overlay, LLM adapters (Gemini, MiniMax), auth, MCP client, tool registry |
| `gestalt_cli` | bin | CLI with `run`, `repl`, `serve`, `exec`, `git` commands |
| `gestalt_swarm` | bin | Parallel agent coordinator (legacy, not in default workspace) |
| `synapse-agentic` | lib | Actor model (Hive), LLM providers, tool primitives |

### gestalt-router modules

| Module | Description |
|--------|-------------|
| `agent` | `SubprocessRunner` — spawns and manages CLI agent processes with POSIX process group kill and timeout |
| `checkpoint` | Git-based checkpoint per agent with symlink escape detection and ignored file filtering |
| `integrate` | Sequential branch integration using `git merge-tree` with binary conflict detection |
| `overlap` | File overlap detection between agent branches using `git diff --name-only` |
| `worktree` | `WorktreeManager` — lifecycle for git worktrees (create, list, remove, prune) |
| `run` | Core types: `RunSpec`, `RunReport`, `AgentResult`, `RouterError` |
| `run_state` | Run manifest and agent state transitions |
| `timeline` | JSONL event log with `Event` enum for tracking run lifecycle events |
| `router` | `Router::execute()` — main pipeline orchestrator |
| `doctor` | Cleanup and recovery for orphaned runs |
| `process` | `ProcessManager` with `CancellationToken` and timeout |

---

## 🔑 Architecture: Gestalt Router

The **Router** is the core orchestrator. It manages the full lifecycle of a multi-agent run:

### Pipeline stages

1. **Validate spec** — ensures agents are specified, base_ref resolves
2. **Write manifest** — atomic JSON manifest with per-agent state
3. **Create worktrees** — one `git worktree` per agent, each on its own branch from base_ref
4. **Spawn agents** — parallel execution via `JoinSet` with `Semaphore`-bounded concurrency
5. **Checkpoint** — each agent's changes committed with symlink-escape detection
6. **Detect overlaps** — compare file lists between agents to find merge conflicts early
7. **Integrate** — sequential merge of agent branches using `git merge-tree --write-tree`
8. **Cleanup** — remove all worktrees
9. **Report** — `RunReport` with merged branches and conflicts

### Agent isolation model

```
agent-a worktree/      agent-b worktree/      agent-c worktree/
    ├── src/               ├── src/               ├── src/
    ├── Cargo.toml         ├── Cargo.toml         ├── Cargo.toml
    └── ...                └── ...                └── ...
        ▲                      ▲                      ▲
        │                      │                      │
        └──────────────┬───────┴───────────┬──────────┘
                       │                   │
              base_ref commit        base_ref commit
                   (main)               (main)
```

Each agent gets their own branch + worktree. After all agents finish, branches are merged sequentially back to `base_ref`.

---

## 📋 Commands

### gestalt run
```bash
gestalt run [OPTIONS]

Options:
  --task <TEXT>              Task description for agents
  --agents <LIST>            Comma-separated list of agent commands
  --base-ref <REF>           Git base ref (default: "main")
  --max-parallel <N>         Max parallel agents (default: 4)
  --timeout <SECONDS>        Per-agent timeout (default: 300)
```

### gestalt repl
Interactive REPL with git operations, tool execution, and task management.

### gestalt serve
Start MCP server for external tool integration.

---

## 🧪 Development

```bash
# Check compilation
cargo check --workspace

# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace

# Format
cargo fmt --all
```

### Requirements
- Rust 2021+ edition
- Git 2.38+ (for `merge-tree --write-tree` support)

---

## 📄 License

MIT — See [LICENSE-MIT](LICENSE-MIT)

---

*Built with ❤ at SouthWest AI Labs*
