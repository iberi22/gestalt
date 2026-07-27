# gestalt-swarm Skill

## Descripción
Gestalt Swarm — Parallel execution bridge for multi-agent orchestration. Dispatch N agents concurrently for high-speed automation.

## Uso

```bash
# Build
cargo build --release -p gestalt_swarm

# Run with N agents and a goal
cargo run --release -p gestalt_swarm -- --agents 4 --goal "analyze codebase"

# With custom model and concurrency
cargo run --release -p gestalt_swarm -- --agents 8 --goal "security audit" --model MiniMax-Text-01 --max-concurrency 16

# Quiet mode (less output)
cargo run --release -p gestalt_swarm -- --agents 4 --goal "scan files" --quiet
```

## Configuración

```bash
# Required
--agents N          # Number of parallel agents (default: 4)
--goal "<task>"      # Task/goal for all agents (required)

# Optional
--model <model>     # Model name (default: MiniMax-Text-01)
--max-concurrency N # Max concurrent LLM calls (default: 8)
--cwd <path>        # Working directory (default: current dir)
--quiet, -q         # Less output
```

## Estado

✅ Operativo — `gestalt_swarm` ejecutable con CLIargs parsing, tokio async, semaphore concurrency, y summary reporting

---

*Gestalt Swarm — Parallel AI agents that actually execute.*
