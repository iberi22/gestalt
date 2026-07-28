# Gestalt Skill

## Descripción
Gestalt es la plataforma de orquestación de agentes AI de SouthWest AI Labs. Workspace Rust con VFS, Swarm, Timeline y Tool Registry.

## Ubicación
`E:\scripts-python\gestalt-rust`

## Crates (workspace)

| Crate | Binary | Descripción |
|-------|--------|-------------|
| gestalt_core | lib | VFS, auth, LLM, tools, MCP client |
| gestalt_cli | `gestalt_cli` | REPL + swarm CLI |
| gestalt-router | lib | Orchestration / run states |
| gestalt-merge | lib | Isolated branch merge |
| gestalt-state | lib | SQLite + MemState |
| gestalt-ws | lib | WebSocket timeline broadcast |
| synapse-agentic | lib | Tool registry + Hive |

`gestalt_swarm` está **excluido** del workspace (legacy). No usar `cargo -p gestalt_swarm`.

## Uso

### CLI
```bash
cargo run -p gestalt_cli
```

### Build
```bash
cargo build --release -p gestalt_cli
cargo check --workspace
```

## Configuración

```bash
export GESTALT_DATABASE_URL="surrealdb:memory"
export GESTALT_LLM__OPENAI__API_KEY="sk-..."
```

Ver `gestalt_core/src/application/CONFIG.md` para todas las variables.

## Herramientas Disponibles (12+)

- `scan_workspace` — directory tree
- `search_code` — vector similarity search
- `execute_shell` — shell commands
- `read_file` / `write_file`
- `git_status` / `git_log` / `git_branch` / `git_add` / `git_commit` / `git_push`
- `clone_repo` / `list_repos`
- `ask_ai` — query LLM

## Swarm

```bash
# Prefer Python bridge or gestalt_cli (gestalt_swarm crate excluded from workspace)
python scripts/swarm_bridge.py --goal "analyze codebase"
cargo run -p gestalt_cli -- swarm --help
```

## MCP Client

Gestalt tiene un MCP client (no server) que puede conectarse a servers MCP externos.
Configurar en `config/mcp.json`.

## Autenticación

Google OAuth2 + PKCE disponible en `gestalt_core/adapters/auth/`.

---

*SouthWest AI Labs — AI agents that actually execute.*
