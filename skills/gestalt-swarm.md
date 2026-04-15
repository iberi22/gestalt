---

name: gestalt-swarm
description: Gestalt Swarm — Parallel Agent Execution Bridge. Launch N CLI agents in parallel via exec, parse JSON results. Use gestalt_swarm_launch(goal, max_agents?) from any OpenClaw session. Repo: iberi22/gestalt-rust
metadata: {"openclaw":{"emoji":"🐝","requires":{"anyBins":["python3","rg","git","cargo"]}}}}

---

# Gestalt Swarm — OpenClaw Skill

> **⚡ Gestalt como tool: ejecuta N agentes en paralelo vía exec directo.**

## Tool Invocation (LLM → Tool)

When the LLM needs to run Gestalt Swarm, it MUST use the `exec` tool with the following pattern:

```
exec: python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "..." --max-agents N --json
```

**Parameters:**
- `goal` (string, required) — Human-readable goal description
- `max_agents` (int, optional, default=10) — Max agents to run in parallel
- `agents` (string, optional) — Comma-separated agent IDs to run (instead of all)
- `--json` (flag, required) — Output machine-readable JSON

**Examples:**

```bash
# Run up to 10 agents in parallel
exec: python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "analyze gestalt-rust for errors" --max-agents 10 --json

# Run specific agents only
exec: python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "find all TODOs" --agents "find_todos,security_audit" --json

# Quick status (3 agents, ~91ms)
exec: python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "quick status" --agents "git_analyzer,git_status,env_check" --json
```

## How Results Are Parsed

The LLM parses the JSON output:

```json
{
  "goal": "...",
  "duration_ms": 91,
  "stats": { "total": 3, "successful": 3, "warnings": 0, "errors": 0 },
  "agents": [
    { "id": "git_analyzer", "name": "Git Analyzer", "status": "success", "duration_ms": 63, "stdout": "..." },
    { "id": "git_status",   "name": "Git Status",   "status": "success", "duration_ms": 86, "stdout": "..." },
    { "id": "env_check",    "name": "Env Checker",   "status": "success", "duration_ms": 66, "stdout": "..." }
  ]
}
```

**Status values:** `success` | `warn` | `error` | `timeout`

The LLM consolidates results and reports a summary to the user.

## Available Agents (15)

| Agent ID | Name | Command | Timeout |
|----------|------|---------|---------|
| `code_analyzer` | Code Analyzer | `rg -c . <dir> -g *.rs` | 15s |
| `dep_check` | Dependency Check | `cargo tree --depth 1` | 30s |
| `cargo_check` | Cargo Check | `cargo check --message-format=short` | 60s |
| `test_runner` | Cargo Check | `cargo check --message-format=short` | 60s |
| `git_analyzer` | Git Analyzer | `git log --oneline -20` | 10s |
| `git_status` | Git Status | `git status --short` | 5s |
| `file_scanner` | File Scanner | `rg --files <dir>` | 10s |
| `log_parser` | Log Parser | `rg "ERROR" <dir> --type log -l` | 10s |
| `security_audit` | Security Audit | `rg "TODO|FIXME|XXX|unsafe" <dir>` | 15s |
| `metrics` | Cargo Stats | `cargo tree --depth 2` | 20s |
| `doc_gen` | Doc Generator | `rg --type md -l . <dir>` | 10s |
| `api_tester` | API Tester | `curl http://localhost:8003/health` | 5s |
| `find_todos` | TODO Finder | `rg "TODO|FIXME|HACK" <dir> -n` | 10s |
| `rust_files` | Rust Files | `rg --files <dir> --type rs` | 10s |
| `env_check` | Env Checker | `rg "^[^#]" .env.example` | 5s |

## Architecture

```
┌─────────────────────────────────────────────┐
│  OpenClaw LLM (GPT/MiniMax)                 │
│    →gestalt_swarm_launch(goal, max_agents?) │
├─────────────────────────────────────────────┤
│  Gestalt Swarm Skill                        │
│    → spawns: python swarm_bridge.py --goal  │
│    → parse JSON results                    │
│    → returns consolidated output          │
├─────────────────────────────────────────────┤
│  swarm_bridge.py (asyncio parallel exec)    │
│    → asyncio.gather(N execs)               │
│    → each: rg, cargo, git, curl, etc.       │
│    → returns JSON {agents: [...]}          │
└─────────────────────────────────────────────┘
```

## Performance

**3 agentes en ~91ms** (secuencial sería ~300ms+):
- `git_analyzer`: 63ms
- `git_status`: 86ms
- `env_check`: 66ms

Los 3 corren en paralelo real via `asyncio.gather()`.

## CLI Options Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--goal` | Goal description (required) | — |
| `--max-agents N` | Max agents to run (1-N) | 10 |
| `--agents IDS` | Comma-separated agent IDs | All available |
| `--json` | Output JSON only | false |
| `--quiet` | Minimal output (status icons) | false |

## Files

- `E:\scripts-python\gestalt-rust\swarm_bridge.py` — Python bridge
- `E:\scripts-python\gestalt-rust\Cargo.toml` — workspace config
- `C:\Users\belal\clawd\skills\gestalt-swarm\SKILL.md` — this skill

## Status

✅ Python bridge working (263ms for 2 agents parallel, verified)
✅ 15 real CLI agents defined
✅ Skill registered with OpenClaw
✅ Tool invocation documented for LLM
