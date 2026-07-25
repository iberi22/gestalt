# Architecture Design

## Arquitectura Post-Refactor (sin SurrealDB)

```
┌──────────────────────────────────────────────────┐
│                 gestalt (CLI entry)               │
├──────────────────────────────────────────────────┤
│                                                  │
│  ┌────────────────┐  ┌────────────────────────┐  │
│  │  WaveManager   │  │     AgentRunner         │  │
│  │                │  │                         │  │
│  │ • create_wave  │  │ • spawn(agent_id, task) │  │
│  │ • snapshot_base│  │ • inyecta worktree_path │  │
│  │ • collect_diffs│  │ • soporta: agy, kimi,   │  │
│  │ • merge_wave   │  │   codex, jules, claude  │  │
│  └───────┬────────┘  └────────┬───────────────┘  │
│          │                    │                  │
│  ┌───────┴────────────────────┴───────────────┐  │
│  │              BranchManager                  │  │
│  │                                              │  │
│  │  • snapshot_base(repo, wave_id) → Snapshot   │  │
│  │  • create_agent_branch(snapshot, id) → Path  │  │
│  │    (usa git worktree o directory copy)       │  │
│  │  • diff_agent(workspace) → Vec<FileDiff>     │  │
│  │  • cleanup_wave(wave_id)                     │  │
│  └──────────────────┬───────────────────────────┘  │
│                     │                              │
│  ┌──────────────────┴───────────────────────────┐  │
│  │               MergeEngine                     │  │
│  │                                               │  │
│  │  • three_way_merge(base, ours, theirs) → str  │  │
│  │    (usa threeway_merge crate)                 │  │
│  │  • batch_merge(base, agents_diffs) → Report   │  │
│  │  • resolve_auto() → sin conflict markers      │  │
│  │  • resolve_llm() → LLM-assisted resolution    │  │
│  │  • flag_manual() → conflictos que escalan     │  │
│  └──────────────────────────────────────────────┘  │
│                                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │           FileManager (EXISTENTE)             │  │
│  │  (actor mpsc, version tracking, locks, patch) │  │
│  │  SE MANTIENE IGUAL — sin cambios              │  │
│  └──────────────────────────────────────────────┘  │
│                                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │         Xavier CLI Bridge (NUEVO)             │  │
│  │  • GET http://localhost:8006/memory/search    │  │
│  │  • POST http://localhost:8006/memory          │  │
│  │  • En lugar de SurrealDB queries              │  │
│  └──────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

## Flujo Completo: Una Wave de 15 Tareas

```
FASE 0: PREPARACIÓN
────────────────────
1. Leer features.json y SRS/SRC docs
2. Crear 15 issues con [file único por issue]
3. Validar que ningún par de issues toca el mismo archivo
   → Si hay overlap, ponerlos SECUENCIALES en la wave
4. git checkout main && git pull
5. COMMIT BASE: git commit --allow-empty -m "wave-N base"

FASE 1: BRANCHING
──────────────────
6. Por cada issue:
     git worktree add /tmp/gestalt/wave-N/agent-{id} HEAD
7. Asignar a cada agente su worktree_path

FASE 2: EJECUCIÓN PARALELA
───────────────────────────
8. Lanzar agentes (agy, kimi, etc.) apuntando a su worktree
9. Cada agente solo ve SU worktree — sin contaminación
10. Al terminar cada agente:
      cd /tmp/gestalt/wave-N/agent-{id}
      git add -A && git commit -m "feat(wave-N): issue description"

FASE 3: COLLECT DIFFS
───────────────────────
11. Por cada agente completado:
      git diff <base-commit> HEAD > /tmp/patches/agent-{id}.diff

FASE 4: MERGE
──────────────
12. Para cada archivo tocado por >1 agente:
      threeway_merge::merge_strings(base, ours, theirs)
13. Archivos tocados por 1 solo agente → aplicar directamente
14. Conflictos detectados:
      - Union mode → auto-merge (toma ambos cambios)
      - Si aun así hay conflictos → LLM-assisted resolution
15. Escribir archivos mergeados al workspace main

FASE 5: VERIFICACIÓN
─────────────────────
16. cargo check --workspace
17. cargo clippy
18. cargo test --lib
19. Si pasa → commit + push
20. Si falla → identificar agente culpable, corregir, reintentar

FASE 6: CLEANUP
────────────────
21. git worktree remove /tmp/gestalt/wave-N/agent-*
22. rm -rf /tmp/gestalt/wave-N/
23. Cerrar issues
```

## Estrategia Anti-Conflicto (REVISADA Jul 2026)

### Stack completo de prevención

| Nivel | Técnica | Efectividad |
|-------|---------|-------------|
| 1 | **File-Island**: 1 issue = 1 archivo | Si se cumple → 0 conflictos |
| 2 | **Weave entity merge**: Diferentes funciones en mismo archivo | 31/31 escenarios |
| 3 | **Weave MCP**: Claim entities antes de editar | Previene overlaps |
| 4 | **Sequential merges**: Mergear branches uno por uno | Merge tax lineal |
| 5 | **LLM-assisted resolution**: Para conflictos reales | Fallback final |

### Weave — Entity-Level Merge Driver

https://github.com/ataraxy-labs/weave · `cargo install weave`

Reemplaza merge de líneas por merge de entidades (funciones, clases,
structs) usando tree-sitter. Endorsado por Elijah Newren (autor del
merge-ort de Git).

```bash
weave setup  # configura .gitattributes + .gitconfig para el repo
# A partir de aquí, git merge usa Weave automáticamente
```

| Escenario | Git (line-level) | Weave (entity-level) |
|-----------|-----------------|---------------------|
| Agentes agregan diferentes funciones al mismo archivo | ❌ CONFLICTO | ✅ AUTO-MERGE |
| Misma función modificada por ambos | ❌ CONFLICTO | ✅ INTRA-ENTITY MERGE |
| Uno modifica, otro elimina | ❌ CONFLICTO confuso | ✅ MENSAJE CLARO |
| 31 escenarios benchmark | 15/31 | **31/31** |

### Merge Tax: Sequential Merges

Para evitar overhead cuadrático:

```
CORRECTO:
main ── merge A ── rebase B ── merge B ── rebase C ── merge C

INCORRECTO:
main ── merge A+B+C (N*(N-1)/2 conflictos potenciales)
```

### Concurrencia

- **4-8 agentes simultáneos** máximo
- **100+ tareas en cola**, 4-8 ejecutándose
- Mismo archivo → NUNCA en paralelo
