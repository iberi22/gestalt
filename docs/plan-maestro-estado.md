# 🏁 GESTALT ECOSYSTEM — Plan Maestro de Estado

> Actualizado: 2026-07-27
> GitHub: iberi22/gestalt @ d38be7d (407 commits)
> Xavier: healthy ✅ (embeddings nomic-embed-text via Ollama)

---

## 📊 Resumen de Componentes

```
═══════════════════════════════════════════════════════════════
 COMPONENTE               ESTADO     MÉTRICA
═══════════════════════════════════════════════════════════════
 Xavier Memory            ✅ healthy  embeddings, search, add
 Xavier Embeddings        ✅ healthy  nomic-embed-text, 480ms
 Gestalt CLI              ✅ compila  4 comandos (search/add/cycle/stats)
 Agent Registry           ✅ 0 errors  12 agentes, TPM+Providers+Ocupado
 BlockEditing             ✅ 0 errors  AgentWrapper 925 LOC, 5 tests
 VFS Tool                 ✅ 676 lines create/run/capture/destroy/status
 gestalt xavier cycle     ✅ 0 errors  --vfs-dir / --overlay-dir
 Workspace                ✅ 0 errors  7/8 crates (gestalt_swarm excluido)
 Git hooks                ✅ 4 hooks   pre-commit, commit-msg, post-commit, pre-push
 Hermes skills            ✅ 2 skills  orchestration + xavier-cycle
 GitHub Issues            27 closed   12 open
═══════════════════════════════════════════════════════════════
```

---

## ✅ COMPLETADO (Waves 1-4 + Afinación)

### Wave 1 — Fundación
- [x] Análisis de código Gestalt (55 APIs)
- [x] Indexación de docs en Xavier
- [x] GLOBAL_GOAL.md creado

### Wave 2 — Zero Conflict Protocol
- [x] Fix 4 test files (agent_tests, doctor_tests, router_tests, integration_test)
- [x] Fix test_try_lock_exclusive (deadlock DashMap)
- [x] G4 LiveConflictDetector + WsEvent
- [x] Push a GitHub con historial limpio

### Wave 3 — Tool Xavier + Agent Registry
- [x] gestalt xavier CLI (search, add, cycle, stats)
- [x] gitcore integration (branch, commits, diff)
- [x] Agent Registry (12 agentes TOML + Rust)
- [x] Rate Limiter (RPM/TPM + 4 routing strategies)
- [x] Tiny Agents (4 modelos diminutos)
- [x] Skills: gestalt-xavier-cycle, gestalt-orchestration

### Wave 4 — BlockEditing + Hooks + Protocolo
- [x] AgentWrapper.execute() real (925 LOC, diff parser + VFS apply)
- [x] Git hooks post-commit + pre-push
- [x] Protocolo Hermes↔Gestalt documentado
- [x] Issues #373-#378 cerradas

### Afinación (Auditoría Kimi K3 + Grok 4.5 High)
- [x] Cycle VFS isolation + full content archiving
- [x] TPM tracking (antes muerto, ahora funcional)
- [x] Provider rate limits (antes inertes, ahora trackeados)
- [x] Tiny selection bug fix (keyword matching)
- [x] Ocupado timeout recovery (60s auto)
- [x] Hardcoded Xavier token eliminado
- [x] XavierClient::new() retorna Result (antes panics)
- [x] Registry NaN unwrap fix (unwrap_or(Equal))
- [x] Registry mark_available() limpia ocupado_desde
- [x] SKILL.md vs TOML consistentes (hermes 200 RPM)
- [x] AgentWrapper tests (5 tests: diff parser + split_command)
- [x] XavierClient memory kinds (save_plan, save_execution, save_config, etc.)
- [x] gestalt-merge + synapse-agentic compilan (0 errores)
- [x] AGENTS.md actualizado (7/8 crates activos)
- [x] Xavier embeddings HEALTHY (local-gllm + Ollama)
- [x] gestalt-vfs-env.sh (herramienta VFS para entornos aislados)
- [x] gestalt xavier cycle --vfs-dir / --overlay-dir

---

## 🔄 PENDIENTE

### Issues Abiertas en GitHub (12)

```
 #336 [JULES-03] Checkpointer + OverlapDetector review     → seguridad
 #337 [JULES-04] integrate_branches + Timeline review       → merge flow
 #338 [JULES-05] Router::execute full pipeline review       → orquestación
 #339 [HARDEN] Eliminar unsafe transmute en Router          → seguridad
 #340 [HARDEN] Consolidar Router implementations            → refactor
 #341 [HARDEN] Replace unsafe pre_exec setsid               → seguridad
 #349 gestalt-merge Cargo dependencies                       → compilación
 #350 gestalt-merge lib.rs                                   → módulos
 #351 gestalt-merge error.rs                                 → errores
 #352 gestalt-merge types.rs                                 → tipos
```

### Próximos Pasos Recomendados

| Prioridad | Tarea | Esfuerzo | Dependencias |
|-----------|-------|----------|-------------|
| 🔴 Alta | Probar merge flow gestalt-merge con router real | 2-4h | Issues #349-#352 |
| 🔴 Alta | Cerrar issues #349-#352 (ya compilan) | 30min | Ninguna |
| 🟡 Media | HARDEN review (#336-#341) con Jules | 4-8h | Acceso Jules |
| 🟡 Media | Reindexar documentos en Xavier con embeddings | 1h | Xavier healthy ✅ |
| 🟢 Baja | Tiny agents fine-tuning (dataset + LoRA) | 8-16h | GPU, dataset |
| 🟢 Baja | Dashboard Web (UI monitoreo agentes) | 4-8h | Ninguna |
| 🟢 Baja | CLI autocomplete (bash completion) | 1h | Ninguna |

---

## 📁 Documentos Indexados en Xavier

| Documento | Kind | Path |
|-----------|------|------|
| ARCHITECTURE.md | plan | gestalt/docs/architecture |
| AGENTS.md | plan | gestalt/docs/agents |
| GLOBAL_GOAL.md | plan | gestalt/docs/global-goal |
| gestalt-orchestration skill | plan | hermes/skills/gestalt-orchestration |
| agent-registry.toml | config | gestalt/agent-registry |
| hermes-gestalt-protocol | plan | gestalt/docs/protocol |
| kimi-k3-feedback | plan | gestalt/docs/kimi-k3-feedback |
| fase-de-afinacion | plan | gestalt/docs/fase-de-afinacion |
| embedding-fix | test | gestalt/test/embedding-fix |
| Kanban Board | plan | swal/kanban/kanban-board |
| SWAL Roadmap | plan | swal/roadmap/roadmap-2026 |

---

## 🏗️ Arquitectura Actual

```
╔══════════════════════════════════════════════════════════════╗
║                    USUARIO (BELA)                          ║
║              pide tarea → recibe resultado                  ║
╚══════════════════════════════════════════════════════════════╝
                      ↓
╔══════════════════════════════════════════════════════════════╗
║            HERMES (orquestador estratégico)                 ║
║    Skills · memoria · delegación · decide qué agente usar   ║
╚══════════════════════════════════════════════════════════════╝
          ↓               ↓               ↓
╔══════════════╗  ╔══════════════╗  ╔══════════════════════════╗
║  Grok 4.5 H  ║  ║   Kimi K3   ║  ║  agy (Gemini 3.6 Flash)  ║
║  planificar  ║  ║  analizar   ║  ║  editar, code-review     ║
╚══════════════╝  ╚══════════════╝  ╚══════════════════════════╝
          ↓               ↓               ↓
╔══════════════════════════════════════════════════════════════╗
║            GESTALT (backend técnico)                        ║
║  VFS · BlockEdit · Conflictos · Rate Limits · Agent Registry║
║  CLI: gestalt xavier search|add|cycle|stats                 ║
║  Tool: gestalt-vfs-env.sh create|run|capture|destroy        ║
╚══════════════════════════════════════════════════════════════╝
          ↓               ↓               ↓
╔══════════════╗  ╔══════════════╗  ╔══════════════════════════╗
║   Ollama     ║  ║   Xavier    ║  ║    GitCore / GitHub      ║
║  nomic-embed ║  ║  memoria    ║  ║    control de cambios    ║
║  qwen3-coder ║  ║  persistente║  ║    issues + PRs          ║
╚══════════════╝  ╚══════════════╝  ╚══════════════════════════╝
```

---

## 🐳 Comandos Rápidos

```bash
# Estado de Xavier
curl -s http://localhost:8006/health | python3 -m json.tool

# Buscar en Xavier
curl -s -X POST http://localhost:8006/v1/memories/search \
  -H "Content-Type: application/json" \
  -H "X-Xavier-Token: $XAVIER_TOKEN" \
  -d '{"query":"gestalt","limit":5}'

# Gestalt CLI
gestalt xavier stats
gestalt xavier search "arquitectura"
gestalt xavier add "resultado" --path "gestalt/runs/test"
gestalt xavier cycle "fix" --agent "cargo check" --vfs-dir . --overlay-dir /tmp/over

# VFS tool
gestalt-vfs-env.sh create ~/projects/mi-proyecto
gestalt-vfs-env.sh run "cargo check"
gestalt-vfs-env.sh capture
gestalt-vfs-env.sh destroy

# Compilar workspace completo
unset OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR
PKG_CONFIG_PATH="$(nix eval nixpkgs#openssl.dev --raw)/lib/pkgconfig"
cargo check --workspace --exclude gestalt_swarm

# Tests
cargo test -p gestalt_core
cargo test -p gestalt_cli
```
