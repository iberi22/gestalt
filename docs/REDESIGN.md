# Gestalt Redesign — Multi-Agent Codebase Router

> **Documento de Diseño** · Julio 2026
> **Basado en:** Investigación AGY (Gemini 3.6 Flash) + Validación Kimi (k3 high thinking)

---

## 1. ¿Qué es Gestalt?

Gestalt es un **orquestador de agentes CLI** que permite a múltiples agentes de código (agy, claude-code, opencode, gemini) trabajar sobre el mismo repositorio simultáneamente, gestionando el aislamiento de cambios, la detección de conflictos y la generación de ramas Git mergeables.

**NO es:** Un framework de agentes, un LLM provider, una base de datos vectorial, un servidor MCP, ni un sustituto de Git.

**SÍ es:** Un router que recibe un objetivo ("refactor auth module"), lanza N agentes CLI en worktrees aislados, recolecta los cambios, detecta solapamientos y produce ramas Git + PRs.

---

## 2. Validación Arquitectónica (Kimi, Jul 2026)

### Decisiones confirmadas ✅

| Decisión | Estado | Por qué |
|----------|--------|---------|
| Git Worktree por agente | ✅ Adoptado | Aislamiento POSIX nativo, cero overhead, `discard()` = `git clean -fdx` |
| Semantic 3-Way Merge | ✅ Adoptado (Fase 2) | AST tree-sitter + diff línea como fallback |
| Post-hoc overlap detection | ✅ Adoptado (v1) | OCC después de ejecución, no antes |
| JSONL event log | ✅ Adoptado | Reemplaza SurrealDB. Append-only, consultable con `jq` |
| Git como timeline | ✅ Adoptado | Commits + branches + merges = historial completo |

### Decisiones rechazadas ❌

| Decisión | Rechazado | Alternativa |
|----------|-----------|-------------|
| FUSE Daemon | ❌ Overkill v1 | Worktree CWD da el mismo aislamiento sin overhead |
| SurrealDB Timeline | ❌ 31 errores compilación | JSONL + git history. SurrealDB era la herramienta incorrecta |
| Lock Server | ❌ Innecesario | PathClaims in-process + detección post-hoc (v1) |
| gestalt_timeline repair | ❌ 31 errores en `Thing` | Reemplazar con `gestalt-router` + módulo timeline |

---

## 3. Estructura de Crates

```
gestalt/
├── gestalt_core/          # ≈1,500 lines ✅ COMPILA
│   ├── vfs/              VirtualFileSystem trait, OverlayFs
│   ├── tools/            ToolRegistry, MCP client, LLM adapters
│   └── agent/            Agent logic traits
│
├── synapse-agentic/       # ≈700 lines ✅ COMPILA
│   └── Hive actor model, providers
│
├── gestalt-router/        # ≈1,200 lines NUEVO - Orquestación
│   ├── run.rs             RunSpec, RunHandle, ejecución
│   ├── worktree.rs        WorktreeManager (git worktree lifecycle)
│   ├── agent.rs           AgentRunner trait + SubprocessRunner
│   ├── integrate.rs       Commit → branch → merge secuencial
│   ├── overlap.rs         Path-set overlap detection
│   ├── lock.rs            PathClaims in-process (DashMap)
│   └── timeline.rs        Event enum + JsonlEventLog
│
├── gestalt-merge/         # ≈500 lines NUEVO - 3-way merge
│   ├── engine.rs          MergeEngine trait
│   ├── git_three_way.rs   git merge-tree wrapper (v1)
│   └── semantic.rs        tree-sitter AST merge (Fase 2)
│
├── gestalt_cli/           # ≈300 lines REPARADO
│   └── gestalt run --agents agy,claude "task"
│
├── gestalt_swarm/         # ≈900 lines REPARADO (opcional, Fase 3+)
│
└── gestalt_timeline/      # ELIMINADO - reemplazado por timeline.rs en router
```

---

## 4. API Surface

### RunSpec

```rust
pub struct RunSpec {
    pub base_ref: String,                    // "main"
    pub task: String,                        // objetivo del agente
    pub agents: Vec<AgentSpec>,
    pub integration_branch: String,          // "gestalt/run-{uuid}"
    pub timeout: Duration,
}

pub struct AgentSpec {
    pub id: String,                          // "agent-a"
    pub command: String,                     // "agy" | "claude" | "opencode"
    pub args: Vec<String>,
    pub allowed_paths: Option<Vec<String>>,  // globs opcionales
}
```

### Router

```rust
pub struct Router {
    worktrees: WorktreeManager,
    runner: Box<dyn AgentRunner>,
    merger: Box<dyn MergeEngine>,
    log: Box<dyn EventLog>,
}

impl Router {
    pub async fn execute(&self, spec: RunSpec) -> Result<RunReport, RouterError>;
    // 1. create git worktree per agent from base_ref
    // 2. spawn each agent CLI with CWD=worktree (tokio::JoinSet)
    // 3. on completion: git commit per agent
    // 4. detect path overlap between agent diffs
    // 5. merge agents sequentially into integration branch
    // 6. return RunReport
}

pub struct RunReport {
    pub run_id: Uuid,
    pub agents: Vec<AgentResult>,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<ConflictInfo>,
    pub events_path: PathBuf,
}
```

### Event Log

```rust
pub enum Event {
    RunStarted { run_id: Uuid, agents: Vec<String>, task: String },
    AgentFinished { run_id: Uuid, agent: String,
                    files_changed: Vec<PathBuf>, duration: Duration },
    OverlapDetected { run_id: Uuid, paths: Vec<PathBuf>,
                      agents: (String, String) },
    MergeConflict { run_id: Uuid, agent: String, paths: Vec<PathBuf> },
    BranchPublished { run_id: Uuid, agent: String, branch: String },
}
```

---

## 5. Flujo de Trabajo

```bash
gestalt run --agents agy,claude "Refactor auth module to use X25519"
```

```
1. Router crea worktrees:
   /tmp/gestalt/run-abc123/agent-a  ← git worktree add -b gestalt/run-abc123/a main
   /tmp/gestalt/run-abc123/agent-b  ← git worktree add -b gestalt/run-abc123/b main

2. Router spawns agentes (tokio::JoinSet):
   CWD=/tmp/gestalt/run-abc123/agent-a agy "Refactor auth module..."
   CWD=/tmp/gestalt/run-abc123/agent-b claude "Refactor auth module..."

3. Cada agente completa:
   → git add -A && git commit -m "feat(agent-a): ..."
   → push origin gestalt/run-abc123/a

4. Router detecta overlap:
   → git diff --name-only main..gestalt/run-abc123/a
   → git diff --name-only main..gestalt/run-abc123/b
   → intersect paths → log OverlapDetected

5. Merge secuencial:
   → git checkout -b gestalt/run-abc123 main
   → git merge gestalt/run-abc123/a  (clean → merged)
   → git merge gestalt/run-abc123/b  (conflict → branch preserved, report)

6. RunReport:
   ✅ agent-a (3 files) → merged → branch: gestalt/run-abc123
   ❌ agent-b (2 files) → conflicted → branch: gestalt/run-abc123/b
   📄 events: ~/.gestalt/runs/run-abc123.jsonl
```

---

## 6. Plan de Implementación

### Fase 1 — MVP Router (target: ~1 semana)

| Componente | Esfuerzo | Dependencias |
|------------|----------|-------------|
| `gestalt-router` crate | 3 días | tokio, git2/shell git, serde_json, uuid, thiserror |
| `WorktreeManager` | 0.5 día | `git worktree add` via process::Command |
| `SubprocessRunner` | 1 día | tokio::process::Command, timeout, stdout capture |
| `integrate.rs` + `overlap.rs` | 1 día | git diff --name-only, merge-tree |
| `timeline.rs` | 0.5 día | serde_json + BufWriter |
| CLI integration | 1 día | clap en gestalt_cli, conectar con router |

**Acceptance:** `gestalt run --agents agy,opencode "task"` produce 2 branches, merge limpio si paths distintos, conflicto reportado si mismo archivo.

### Fase 2 — Merge Inteligente

- `gestalt-merge` crate con tree-sitter AST merge
- Auto-resolución de conflictos no-overlap en mismo archivo
- PR creation via `gh` CLI

### Fase 3 — Coordinación Preventiva

- PathClaims OCC antes de ejecución (agentes declaran paths → router asigna/bloquea)
- Repartición de tareas si hay overlap detectado en planificación

### Fase 4 — FUSE Opcional (solo si profiling lo justifica)

- Write interception enforcement
- In-memory COW para repos enormes

---

## 7. Dependencias Clave

```toml
# gestalt-router/Cargo.toml
[dependencies]
tokio = { version = "1", features = ["process", "sync", "macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
# git: usar std::process::Command para v1 (evita git2 native dep)
```

```toml
# gestalt-merge/Cargo.toml (Fase 2)
[dependencies]
git2 = "0.19"                    # solo si merges nativos
tree-sitter = "0.24"            # AST merge
similar = "2"                   # diff línea fallback
```

---

*Este diseño reemplaza los conceptos anteriores de timeline/SurrealDB/FUSE. Ver docs/guides/ARCHITECTURE.md para diagrama del sistema.*

## Refinamientos

Este diseño fue validado y refinado con:
1. **AGY (Gemini 3.6 Flash)** — Web research sobre técnicas 2025-2026
2. **Kimi (k3 high thinking)** — Revisión crítica con 6 fallos detectados y corregidos

Ver `docs/REFINEMENTS.md` para la especificación concreta de Fase 1 MVP, pipeline de ejecución detallado, modelo de eventos, plan de pruebas y referencias a mergiraf/STORM/AgentSpawn para fases posteriores.
