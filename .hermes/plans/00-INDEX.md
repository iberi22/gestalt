# Gestalt Redesign Plan

Plan completo en 6 partes:

```
📁 .hermes/plans/
├── 01-overview-research.md      ← Research findings + context
├── 02-architecture-design.md    ← Arquitectura post-refactor
├── 03-phase1-core-extraction.md  ← Fase 1: Eliminar SurrealDB
├── 04-phase2-branch-merge-engine.md ← Fase 2: BranchManager + MergeEngine
├── 05-phase3-agent-runner-cli.md ← Fase 3: AgentRunner + CLI + Xavier
└── 06-executive-summary.md      ← Timeline, issues, metrics
```

**Lectura recomendada:** 02 → 06 → 01 → 03 → 04 → 05

## Hallazgo Clave

El crate `threeway_merge` (v0.1.19) es EXACTAMENTE lo que necesitamos:
100% compatible con Git, soporta Myers/Patience/Histogram,
diff3/zdiff3 conflict styles, favor modes, y es puro Rust + xdiff.

https://crates.io/crates/threeway_merge
https://docs.rs/threeway_merge

## Decisión Arquitectónica Principal

**NO** reescribir el merge desde cero. Usar:
- `git worktree add` para aislamiento por agente
- `threeway_merge::merge_strings()` para three-way merge
- `git diff` para extraer patches
- FileManager existente para el control de cambios fino por archivo

## Siguiente Paso

¿Procedemos con la implementación? Sugiero:
1. Primero eliminar SurrealDB (Fase 1)
2. Luego implementar BranchManager + MergeEngine (Fase 2)
3. Y finalmente conectar agentes CLI externos (Fase 3)
