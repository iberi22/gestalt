# TODO — Gestalt Router Implementation

> **Última actualización:** 2026-07-25

## 🔴 Fase 1 — MVP Router (1 semana estimada)

### Crates nuevos

- [ ] Crear `gestalt-router/` crate:
  - [ ] `WorktreeManager` — `git worktree add/remove` via process::Command
  - [ ] `SubprocessRunner` — spawn agent CLI, timeout, stdout capture
  - [ ] `integrate.rs` — commit per agent → merge secuencial → report
  - [ ] `overlap.rs` — `git diff --name-only` path intersection
  - [ ] `lock.rs` — in-process PathClaims (DashMap)
  - [ ] `timeline.rs` — `Event` enum + `JsonlEventLog`

### Reparaciones

- [ ] Reparar `gestalt_cli`:
  - [ ] Eliminar dependencia de `gestalt_timeline`
  - [ ] Añadir comando `gestalt run --agents X,Y "task"`
  - [ ] Conectar con `gestalt-router`

- [ ] Reparar `gestalt_swarm`:
  - [ ] Eliminar dependencia de `gestalt_timeline`
  - [ ] Adaptar a nuevo modelo de eventos

### Cargo.toml workspace

- [ ] Agregar `gestalt-router` al workspace
- [ ] Agregar `gestalt-merge` al workspace (Fase 2)
- [ ] Eliminar `gestalt_timeline` del workspace

### Acceptance test

- [ ] 2 agentes en 2 archivos diferentes → merge limpio
- [ ] 2 agentes en mismo archivo → conflicto reportado, nada perdido
- [ ] Event log JSONL generado por run
- [ ] Ramas Git creadas correctamente

## 🟡 Fase 2 — Merge Inteligente

- [ ] Crear `gestalt-merge/` crate:
  - [ ] `GitMergeEngine` — git merge-tree wrapper
  - [ ] `TreeSitterMerger` — AST-aware merge
  - [ ] `similar` fallback para diff línea
- [ ] PR creation via `gh` CLI
- [ ] Auto-resolución de conflictos no-overlap en mismo archivo

## 🟡 Fase 3 — Coordinación Preventiva

- [ ] PathClaims antes de ejecución
- [ ] Repartición de tareas si overlap detectado en planificación
- [ ] Integrar `synapse-agentic` actors si JoinSet se queda corto

## 🔵 Fase 4 — FUSE Opcional

- [ ] Evaluar necesidad con profiling de Fase 1-3
- [ ] Implementar `gestalt-fuse` solo si write enforcement es necesario

---

## Completado ✅

- ✅ Diseño REDESIGN.md completo
- ✅ Validación arquitectónica con AGY + Kimi
- ✅ docs/guides/ARCHITECTURE.md actualizado
- ✅ SRC.md actualizado
- ✅ .gitcore/features.json reescrito
- ✅ .gitcore/planning/ actualizado
- ✅ SurrealDB eliminado del diseño
- ✅ FUSE eliminado del diseño
- ✅ gestalt_timeline eliminado
