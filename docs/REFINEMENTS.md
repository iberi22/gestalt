# Gestalt Router — Refinamientos Finales

> **Síntesis de:** AGY web research + Kimi k3 high thinking review
> **Fecha:** 2026-07-25

Este documento consolida los hallazgos de la investigación web y la revisión arquitectónica de Kimi en refinamientos concretos para el diseño Gestalt Router.

---

## 1. Investigación Web (AGY Gemini 3.6 Flash)

### Estado del Arte 2025-2026: Multi-Agent Codebase Editing

| Técnica | Mecanismo | Ventaja | Limitación |
|---------|-----------|---------|------------|
| **Git Worktrees** | Espacios aislados por rama | ✅ Evita colisiones SO | ❌ No resuelve conflictos semánticos |
| **mergiraf** | AST merge driver (tree-sitter) | ✅ Elimina falsos conflictos línea | ❌ Requiere código sintácticamente válido |
| **STORM / ATM** | Mediación transaccional write-time | ✅ Previene contexto contaminado | ❌ Mayor latencia en coordinación |
| **CodeCRDT** | Estado compartido CRDT | ✅ Convergencia garantizada | ❌ 5-10% conflictos semánticos residuales |
| **AgentSpawn** | Merge 3-capas (estructural → LLM → escalación) | ✅ Resuelve conflictos lógicos | ❌ Consumo de tokens alto |

### Consenso de la Industria

La arquitectura multi-agente más sólida en 2026 combina **3 capas**:

1. **Aislamiento Físico** — Git Worktree por agente
2. **Control Transaccional** — Orquestador gestiona conjuntos R/W a nivel AST
3. **Fusión en 2 Fases** — Merge estructural (tree-sitter) → LLM supervisor

### Recomendaciones para Gestalt

- **mergiraf** (https://mergiraf.org) es exactamente lo que `gestalt-merge` debería ser: un driver de merge tree-sitter listo para usar. Evaluar si podemos wrappearlo vs implementar desde cero.
- El modelo 3-capas de **AgentSpawn** (arXiv:2601.12930) valida nuestra estrategia de merge secuencial + AST + escalación a LLM.
- **STORM** (arXiv:2605.20563) sugiere que deberíamos considerar mediación write-time en Fase 3, no solo post-hoc.

---

## 2. Revisión Kimi — 6 Fallos Críticos del Diseño Original

### ❌ FALLO 1: El paso 5 toca el working tree del usuario

**Problema:** `git checkout -b gestalt/run-abc123 main` se ejecuta en el checkout del usuario. Si tiene cambios sin commitear o está en otra rama, el router los pisotearía.

**Refinamiento:** La integración ocurre en un **worktree de integración dedicado** (`run-abc123/_integrate`), creado desde el mismo SHA base. El checkout del usuario nunca se toca. Para el merge sin checkout: usar `git merge-tree --write-tree` (git ≥ 2.38) que calcula el merge en memoria.

**Nuevo flujo de integración:**
```
tree₀ = base (SHA concreto)
tree₁ = merge-tree(tree₀, branch-a) → conflicto? reportar
tree₂ = merge-tree(tree₁, branch-b) → conflicto? reportar
...
git commit-tree tree₂ -p base -p branch-a -p branch-b → merge commit
git update-ref refs/heads/gestalt/run-abc123 <merge-sha>
```

### ❌ FALLO 2: `base_ref` se resuelve N veces

**Problema:** Si `base_ref = "main"` y alguien pushea mientras se crean worktrees, los agentes parten de commits distintos.

**Refinamiento:** El router resuelve `base_ref` a un **SHA concreto una sola vez** con `git rev-parse base_ref^{commit}` al inicio del run, lo registra en el evento `RunStarted`, y crea todos los worktrees desde ese SHA.

### ❌ FALLO 3: `git add -A` pierde archivos ignorados

**Problema:** `git add -A` respeta `.gitignore`. Si un agente crea un archivo nuevo que matchea un gitignore pattern, el cambio se pierde silenciosamente del diff y del merge.

**Refinamiento:** El checkpoint del router usa `git add -A --force` sobre los archivos registrados en el diff, o mejor: captura `git status --porcelain`, hace `git add` de cada archivo no-ignored individualmente, y logea los ignored como eventos `ExcludedFile`.

### ❌ FALLO 4: Git hooks rompen el checkpoint

**Problema:** Si el repo tiene hooks pre-commit que hacen `exit 1`, el commit del router falla y el run queda en estado indefinido.

**Refinamiento:** Router-owned commits usan `--no-verify` + `-c core.hooksPath=/dev/null`.

### ❌ FALLO 5: Sin modelo de estados ni recuperación

**Problema:** Si un agente crashea a mitad, el run no tiene estado definido. Si el router crashea, quedan worktrees y ramas huérfanas.

**Refinamiento:** Modelo de estados por agente + manifiesto de run persistente:

```
AgentState: Pending → Running → Success | Timeout | Crashed | NoChanges | Quarantined
```

Manifiesto: `~/.gestalt/runs/<run_id>/manifest.json` escrito ANTES de crear cualquier recurso, actualizado en cada transición.

Comando de recovery: `gestalt doctor --prune` que lista y limpia runs huérfanos.

### ❌ FALLO 6: Symlinks rompen el aislamiento

**Problema:** Un agente puede crear un symlink que apunte fuera del worktree (`ln -s /etc/passwd`) y leer/escribir archivos fuera del repositorio.

**Refinamiento:** En el checkpoint, el router escanea `git ls-files -s` buscando mode 120000 (symlink) con target fuera del worktree. Si detecta symlink escape: evento `SymlinkEscape` + el cambio va a una rama de cuarentena, no a integración.

---

## 3. Especificación Concreta — Fase 1 MVP Refinada

### 3.1 Módulos del Router

```
gestalt-router/src/
├── lib.rs              # Router struct, execute()
├── run.rs              # RunSpec, RunHandle, AgentSpec, AgentResult
├── run_state.rs        # AgentState enum, RunManifest (serde JSON)
├── worktree.rs         # WorktreeManager: crear/remover worktrees
├── agent.rs            # SubprocessRunner: spawn, timeout, process group kill
├── checkpoint.rs       # Checkpointer: git add + commit (no-verify, sin hooks)
├── overlap.rs          # git diff --name-only path intersection
├── integrate.rs        # merge-tree pipeline secuencial, commit-tree final
├── timeline.rs         # Event enum + JsonlEventLog
└── doctor.rs           # cleanup/doctor command
```

### 3.2 Pipeline de Ejecución (Fase 1)

```
1. router recibe RunSpec:
   - base_ref: "main", agents: [AgentSpec{id, command, args}]

2. router escribe manifest.json (run_id, SHA base pending)

3. router resuelve SHA base: git rev-parse base_ref^{commit}
   → actualiza manifest con SHA

4. router serializa creación de worktrees:
   for agent in agents:
       git worktree add -b gestalt/{run_id}/{agent.id} \
         ~/.gestalt/runs/{run_id}/wts/{agent.id} {SHA}
   → actualiza manifest con lista de worktrees

5. router spawnea agentes (tokio::JoinSet, semaphore limitado):
   CWD = worktree, env sanitized, timeout configurable
   → pipe stdout/stderr a ~/.gestalt/runs/{run_id}/logs/{agent.id}.log

6. por cada agente que completa:
   a. capturar exit code
   b. checkpoint(): git add (individual, no -A) + git commit --no-verify
   c. si exit != 0 → estado Crashed, commit wip
   d. si no hay cambios → NoChanges, saltar

7. overlap detection:
   for each pair (a,b):
     git diff --name-only {SHA}..gestalt/{run_id}/{a}
     git diff --name-only {SHA}..gestalt/{run_id}/{b}
     intersect → OverlapDetected event

8. integración (en memoria, sin checkout):
   if git >= 2.38:
       tree = SHA base
       for agent in sorted(agents):
           tree = git merge-tree --write-tree tree gestalt/{run_id}/{agent}
           if conflicted → MergeConflict event
       git commit-tree tree -p base -p all-branches
       git update-ref refs/heads/gestalt/{run_id} <final-sha>
   else:
       worktree integración → git merge --no-ff (fallback)

9. si --push: git push origin gestalt/{run_id} (opcional)

10. RunReport: resultados, eventos, ramas, conflictos
```

### 3.3 Modelo de Eventos (refinado)

```rust
pub enum Event {
    RunStarted { run_id, sha_base, agents, task },
    AgentStateChanged { run_id, agent, from: AgentState, to: AgentState },
    CheckpointCommitted { run_id, agent, sha, files_changed: Vec<Path>, warning: Option<String> },
    OverlapDetected { run_id, path, agents: (String, String), kind: OverlapKind },
    MergeComputed { run_id, agent, tree_sha, conflict_count },
    MergeConflict { run_id, agent, path, hunk_kind: ConflictKind },
    SymlinkEscape { run_id, agent, path, target },
    ExcludedFile { run_id, agent, path, reason: String },
    RunFinished { run_id, summary },
    // ConflictKind: Binary | SameHunk | ASTNode | Symlink
}
```

### 3.4 CLI (refinada)

```bash
# Comando principal
gestalt run \
  --base main \
  --agents agy,claude,opencode \
  --task "Refactor auth module" \
  --effort high \
  --max-parallel 3 \
  --timeout 300 \
  [--push]

# Output:
# ✅ agent-a (3 files) → merged → gestalt/run-uuid
# ❌ agent-b (2 files) → conflicted → gestalt/run-uuid/b
# ⚠️ agent-c → crashed (exit 137) → wip commit preservado
# 📄 events: ~/.gestalt/runs/uuid/events.jsonl

# Doctor/cleanup
gestalt doctor                     # list orphaned runs
gestalt doctor --prune             # cleanup all orphaned
gestalt doctor --prune <run_id>    # cleanup specific
```

### 3.5 Plan de Pruebas (Fase 1)

| Test | Escenario | Resultado Esperado |
|------|-----------|-------------------|
| T-01 | 2 agentes, 2 archivos diferentes | Merge limpio, sin conflictos |
| T-02 | 2 agentes, mismo archivo, regiones disjuntas | Merge limpio, PathOverlap logueado |
| T-03 | 2 agentes, misma región | MergeConflict, rama preservada |
| T-04 | Agente con exit 1 tras editar | Quarantined con wip commit |
| T-05 | Agente timeout (sleep 999) | SIGKILL, wip commit |
| T-06 | Symlink escape | SymlinkEscape, no integrado |
| T-07 | Repo con pre-commit hook | Checkpoint funciona (--no-verify) |
| T-08 | Archivo gitignored | ExcludedFile, no perdido |
| T-09 | Router kill a mitad | `gestalt doctor --prune` limpia |
| T-10 | Binario modificado por ambos | HardConflict reportado |

---

## 4. Integración con Hallazgos Web para Fases Posteriores

### Fase 2 — Merge Inteligente

- Evaluar **mergiraf** (https://mergiraf.org) como backend tree-sitter en lugar de implementar desde cero
- Implementar modelo **AgentSpawn 3-tier**: merge-tree → tree-sitter → LLM supervisor
- Auto-resolución de conflictos: si tree-sitter puede mergear, se mergea. Si no, LLM decide. Si el LLM no puede, escalación a humano.

### Fase 3 — Coordinación Preventiva

- Inspirarse en **STORM** (arXiv:2605.20563) para mediación write-time: los agentes declaran qué archivos/símbolos van a tocar, el router asigna/bloquea antes de ejecutar.
- Implementar **AST-level claims** (no solo path-level): el agente declara "voy a modificar la función `login()` en `auth.rs`", el router verifica que ningún otro agente tenga claim sobre ese símbolo.

### Fase 4 — CRDT (solo si necesario)

- Evaluar **CodeCRDT** (arXiv:2510.08920) si los conflictos semánticos residuales superan el 10%.
- Integración: reemplazar worktrees por un canvas CRDT compartido para agentes en tiempo real.

---

*Este documento es el diseño oficial de Gestalt v2.0.0.*
*Ver docs/REDESIGN.md para el diseño base, y docs/guides/ARCHITECTURE.md para diagramas.*
