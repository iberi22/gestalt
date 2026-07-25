# Executive Summary + Timeline

## Resumen: Qué se Construye (VERSIÓN FINAL)

### Cambios vs plan original por la validación:

| Aspecto | Plan original (Fase 1) | Versión final validada |
|---------|----------------------|----------------------|
| Merge engine | `threeway_merge` crate (line-level) | **Weave** (entity-level) + `threeway_merge` fallback |
| Concurrentes | 100+ simultáneos | **4-8 simultáneos**, 100+ en cola |
| Merge strategy | Batch merge todos los branches | **Sequential merges** (uno por uno, rebase) |
| Conflict prevention | Version check only | 5 niveles: file-island → Weave entity → MCP claims → sequential → LLM |
| Worktree path | Cualquiera | `~/gestalt-worktrees/` (sandbox no ve /tmp/) |
| Weave install | No considerado | `cargo install weave` + `weave setup` en el repo |

| Componente | Líneas | Estado |
|------------|--------|--------|
| FileManager (version + locks + patch) | ~1038 | ✅ EXISTE — no tocar |
| OverlayFs (VFS in-memory) | ~500 | ✅ EXISTE — no tocar |
| Eliminar SurrealDB (20+ archivos) | ~-2000 | 🔴 REFACTOR |
| BranchManager (git worktree wrapper) | ~200 | 🆕 NUEVO |
| WaveManager (coordinación + sequential merges) | ~300 | 🆕 NUEVO |
| AgentRunner (spawn agy/kimi/claude) | ~300 | 🆕 NUEVO |
| Weave setup + integración | ~50 | 🆕 NUEVO (config) |
| CLI commands | ~200 | 🆕 NUEVO |

**Total nuevo:** ~1,050 líneas · **Total eliminar:** ~2,000 · **Se mantiene:** ~1,600

## Dependencias Nuevas (FINAL)

```toml
# gestalt_core
threeway_merge = "0.1.19"    # Fallback line-level merge
# Sistema (no Cargo):
cargo install weave           # Entity-level merge driver
weave setup                   # Configurar en el repo
```

## Timeline Revisado

| Fase | Componentes | Esfuerzo |
|------|-------------|----------|
| **Fase 1** | Eliminar SurrealDB + simplificar servicios | 2-3h |
| **Fase 2a** | BranchManager (git worktree wrapper) | 1-2h |
| **Fase 2b** | Weave setup + integración + sequential merges | 1h |
| **Fase 2c** | WaveManager (coordinación completa) | 2h |
| **Fase 3a** | AgentRunner (spawn agy/kimi/claude) | 1-2h |
| **Fase 3b** | CLI commands + wave orchestration | 2h |
| **—** | **Prototipo funcional** | **~10-12h** |
| **—** | Tests + edge cases + polish | +20h |

## Métricas de Éxito

1. **100+ tareas por wave** (4-8 simultáneas, encoladas)
2. **Merge automático >90%** — Weave entity-level + sequential merges
3. **Cero SurrealDB** — 0 dependencias de DB externa
4. **Weave entity merge** → 31/31 escenarios sin falsos conflictos

## Orden de Issues (recomendado)

```
Phase 1 (Core):
  GESTALT-001: Remove SurrealDB from gestalt_timeline
  GESTALT-002: Remove SurrealDB from gestalt_core
  GESTALT-003: Simplify timeline/project/task services
  GESTALT-004: Add threeway_merge dependency
  GESTALT-005: Clean up models (remove SurrealDB types)

Phase 2 (Branch + Merge):
  GESTALT-006: Implement BranchManager (snapshot + worktrees)
  GESTALT-007: Implement MergeEngine (threeway_merge wrapper)
  GESTALT-008: Implement WaveManager (coordination)
  GESTALT-009: Unit tests for MergeEngine

Phase 3 (Agents + CLI):
  GESTALT-010: Implement AgentRunner (spawn external CLI agents)
  GESTALT-011: Xavier CLI Bridge for context/memory
  GESTALT-012: New CLI entry point + wave commands
  GESTALT-013: Integration test: full wave cycle
```

## Métricas de Éxito

1. **100+ tareas por wave** — el sistema soporta lanzar 100+ agentes
2. **Merge automático >90%** — sin conflict markers
3. **Cero SurrealDB** — 0 dependencias de DB externa
4. **Compilación en <30s** — sin SurrealDB, build más rápido
5. **Tests de FileManager siguen pasando** — core intacto

## Files que NO se Tocan

```
gestalt_timeline/src/services/file_manager.rs    ← CORE
gestalt_core/src/ports/outbound/vfs.rs            ← CORE
gestalt_core/src/ports/outbound/vfs.rs            ← CORE
gestalt_swarm/src/main.rs                         ← Opcional
synapse-agentic/                                  ← Opcional (si no se usa)
```
