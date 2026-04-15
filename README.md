# Gestalt

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Git-Core Protocol](https://img.shields.io/badge/Git--Core%20Protocol-v3.5-blueviolet)](AGENTS.md)
[![Open Source](https://img.shields.io/badge/Open%20Source-GitHub-green.svg)](https://github.com/iberi22/gestalt-rust)

**Gestalt** is a universal AI agent platform with two core components:

1. **Gestalt CLI** — Context-aware AI assistant for your terminal
2. **Gestalt Swarm** — Parallel execution bridge for massive automation

## 🐝 Gestalt Swarm (Primary Focus)

> ⚡ **Gestalt Swarm** launches N CLI agents in parallel for high-speed automation.

Gestalt Swarm is a Python bridge that executes multiple CLI agents concurrently via `asyncio`. Each agent runs real commands (ripgrep, cargo, git, curl, etc.) and results are consolidated in JSON.

**Quick Start:**
```bash
# Auto-select agents based on goal (smart selection)
python swarm_bridge.py --goal "security audit of codebase" --json

# Dry run (preview agents)
python swarm_bridge.py --goal "analyze gestalt-rust" --dry-run

# Specific agents (~91ms for 3 parallel)
python swarm_bridge.py --goal "quick status" --agents "git_analyzer,git_status" --json

# Streaming mode (partial results as agents complete)
python swarm_bridge.py --goal "scan files" --watch --timeout 30
```

**Available Agents (15):**
| Agent | Purpose |
|-------|---------|
| `code_analyzer` | Ripgrep patterns in code |
| `dep_check` | Cargo tree dependencies |
| `git_analyzer` | Git log history |
| `git_status` | Working tree status |
| `security_audit` | Find TODO/FIXME/unsafe |
| `find_todos` | Find TODO/FIXME/HACK |
| `api_tester` | Test HTTP endpoints |
| `env_check` | Environment variables |
| ... | and 8 more |

**Performance:**
- 3 agents in **~91ms** (vs ~300ms sequential)
- 15 agents fully parallelizable
- Dynamic count based on rate limits and goal complexity

**OpenClaw Integration:**
Gestalt Swarm is registered as an OpenClaw skill. The LLM can invoke it directly:

```bash
exec: python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "..." --json
```

## 🤖 Gestalt CLI

Context-aware AI assistant that intelligently gathers project context before sending to LLMs.

**Install:**
```bash
cargo install --path gestalt_cli
```

**Use:**
```bash
gestalt --prompt "How do I add a new endpoint to this API?"
```

## 📂 Project Structure

```
gestalt-rust/
├── swarm_bridge.py          # 🐝 Gestalt Swarm (parallel exec)
├── skills/
│   ├── gestalt-swarm.md     # Skill documentation
│   └── jules.md            # Jules AI integration
├── .gitcore/               # Git-Core planning docs
│   ├── ARCHITECTURE.md
│   ├── features.json
│   └── planning/
├── gestalt_cli/            # CLI interface
├── gestalt_core/           # Core domain logic
├── gestalt_swarm/          # Swarm Rust binary
└── synapse-agentic/        # Agent runtime
```

## 🔗 Resources

- **Repository:** https://github.com/iberi22/gestalt-rust
- **Issues:** https://github.com/iberi22/gestalt-rust/issues
- **License:** MIT

## 🏗️ Architecture

See [.gitcore/ARCHITECTURE.md](.gitcore/ARCHITECTURE.md) for detailed system design.

## 🤝 Contributing

1. Fork the repo
2. Create a branch (`git checkout -b feature/amazing`)
3. Commit (`git commit -m 'feat: add amazing feature'`)
4. Push (`git push origin feature/amazing`)
5. Open a Pull Request

---

*Gestalt — AI agents that actually execute.*
