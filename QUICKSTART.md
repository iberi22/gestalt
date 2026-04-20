# 🚀 Quick Start — Gestalt

> **Tiempo estimado: 5 minutos**

Orquestación de agentes AI via CLI. Sin UI, sin setup complejo.

## Prerrequisitos

- Rust 1.75+
- SurrealDB (o usar `surrealdb:memory`)

## Step 1 — Build

```bash
git clone https://github.com/iberi22/gestalt-rust.git
cd gestalt-rust
cargo build --release
```

## Step 2 — Run

```bash
# Orchestrator principal
cargo run --release -p gestalt_timeline --bin gestalt

# O REPL standalone
cargo run --release -p gestalt_cli
```

## Step 3 — Configure

```bash
export GESTALT_DATABASE_URL="surrealdb:memory"
export GESTALT_LLM__OPENAI__API_KEY="sk-..."
export GESTALT_LOG_LEVEL="info"
```

## Step 4 — Swarm

```bash
cargo run --release -p gestalt_swarm -- --agents 4 --goal "<your task>"
```

Ejemplo:
```bash
cargo run --release -p gestalt_swarm -- --agents 4 --goal "analyze codebase security"
```

## Tools Disponibles

12+ herramientas listas para usar:
- git (status, log, branch, add, commit, push)
- shell (execute_shell)
- file (read_file, write_file)
- search (search_code, scan_workspace)
- repo (clone_repo, list_repos)
- ai (ask_ai)

## Documentation

- [README.md](README.md) — overview
- [ARCHITECTURE.md](ARCHITECTURE.md) — arquitectura
- [docs/guides/QUICKSTART.md](docs/guides/QUICKSTART.md) — guía detallada
- [STATE.md](STATE.md) — estado actual
- [TODO.md](TODO.md) — pendientes

---

**¿Problemas?** https://github.com/iberi22/gestalt-rust/issues
