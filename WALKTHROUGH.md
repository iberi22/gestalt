# 🚶 WALKTHROUGH.md — Gestalt Usage Walkthrough

> **Goal:** Run a swarm of agents to analyze, plan, and execute a task — all via CLI.

---

## Step 0 — Prerequisites

```bash
cargo build --release
export GESTALT_DATABASE_URL="surrealdb:memory"
export GESTALT_LLM__OPENAI__API_KEY="sk-..."
```

---

## Step 1 — Run the REPL

```bash
cargo run --release -p gestalt_cli
```

The REPL loads with:
- VFS initialized
- Tool registry registered
- SurrealDB connection active

---

## Step 2 — Run the Orchestrator (gestalt binary)

```bash
cargo run --release -p gestalt_timeline --bin gestalt
```

This starts the timeline service + main orchestrator loop.

---

## Step 3 — Run Swarm (parallel agents)

```bash
cargo run --release -p gestalt_swarm -- --agents 4
```

Swarm spawns N agents, each with a VFS + tool registry.

---

## Step 4 — Execute a Task

From the REPL or timeline, send a goal:

```
> analyze workspace for security issues
```

Gestalt will:
1. Parse the goal
2. Select relevant agents
3. Dispatch to Swarm
4. Collect results
5. Persist events to timeline

---

## Step 5 — Inspect Results

```bash
# Git status
> git status

# Search code
> search "TODO"

# Read file
> read Cargo.toml
```

---

## Step 6 — Persist Timeline

Events are auto-saved to SurrealDB. Query with:

```bash
# Connect to SurrealDB CLI
surreal sql --conn "memory"
```

---

## Architecture in Walkthrough

```
User → gestalt_cli (REPL) → gestalt_core (VFS + tools)
                         ↓
                  gestalt_timeline (orchestrator)
                         ↓
                  gestalt_swarm (parallel agents)
                         ↓
                  synapse-agentic (tool registry)
```

---

## Tools Available

| Command | Action |
|---------|--------|
| `execute_shell` | Run shell |
| `git_status` | Git status |
| `git_log` | Git log |
| `scan_workspace` | Directory tree |
| `search_code` | Vector search |
| `read_file` | Read file |
| `write_file` | Write file |
| `ask_ai` | Query LLM |
| `clone_repo` | Clone repo |
| `list_repos` | List repos |

---

*Walkthrough: 2026-04-16*