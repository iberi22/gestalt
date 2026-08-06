# FEEDBACK WAVE-8 GESTALT — 2026-08-06 (integración con subagente OpenCode qwen3.8-max)

## PRs integrados (8/8 MERGED en main, 16:06Z)
| PR | Feature | Issue | Estado | Merge commit |
|----|---------|-------|--------|--------------|
| #526 | FEAT-GT-039 Observe daemon skeleton (gestalt observe CLI + module tree stubs) | #510 GT-04 | MERGED | eb70080 |
| #527 | FEAT-GT-009 Fix broken test recovery (T1/T2/G3) | #507 GT-01 | MERGED | c94ecbf |
| #520 | FEAT-GT-010 LLM provider resilience: failover, Timeout/Crashed | #509 GT-03 | MERGED | 414db4e |
| #525 | FEAT-GT-041 /proc monitor exact-cmdline agent matching | #511 GT-05 | MERGED | 422c343 |
| #521 | FEAT-GT-040 merge-safe hook injection + settings injectors | #512 GT-06 | MERGED | cc6c279 |
| #524 | FEAT-GT-017 Secret/token filter: redact before Xavier sink + WS | #516 GT-10 | MERGED | 8f83abc |
| #522 | FEAT-GT-033 Flight recorder: reconstruct run by run_id | #517 GT-11 | MERGED | b234a5d |
| #523 | FEAT-GT-034 OTel GenAI interop: BusEvent → OTel conventions | #518 GT-12 | MERGED | 4bba26e |

## CI local (resultados reales)
- 8 merges secuenciales con orden de dependencias (merge order 1-12 respetado)
- Conflictos resueltos tomando fillers del lado de los PRs cuando aplicaba (inject.rs, proc_monitor.rs)
- Fix post-merge: 0c78a74 `fix: adapt run_daemon_loop a API real ProcMonitor::poll` (integración skeleton+filler)
- cargo fmt --all aplicado (fbc5913)
- cargo test --workspace: en ejecución durante el cierre del agente (suite grande — tantivy indexación)
- cargo clippy: revisado
- Push origin/main: OK (0c78a74 == origin)

## PROBLEMAS ENCONTRADOS (hallazgos para próximas waves)
1. **Filler/skeleton pattern conflictivo**: los PRs de la wave-8 son "skeleton + filler" (daemon skeleton, stubs) — al integrarlos, los fillers de PRs paralelos se pisaban (run_daemon_loop vs ProcMonitor::poll). Se resolvió con fix de adaptación post-merge.
2. **Tantivy atomic_write en investigación**: el subagente inspeccionó tantivy-0.22 mmap_directory.rs (atomic_write tempfile_in parent) — contexto para el flight recorder (persistencia atómica de runs).
3. **gestalt_swarm sigue excluido** del workspace (legacy, no build) — confirmado en AGENTS.md.
4. **issues audit abiertos**: #468, #471 (features.json desync — falta gestalt-proto/memory/eventbus/wasm), #473, #474 — pendientes de reconciliación.

## Pendientes para nuevas waves
- [ ] Reconciliar .gitcore/features.json de gestalt (15 features, 3 nuevas al 100%; issues #468/#471)
- [ ] Verificar tests completos del workspace tras la wave (cargo test --workspace completo)
- [ ] FEAT-GT-008 (LiveConflictDetector sin timeouts, #508 GT-02) — sin PR en esta wave
- [ ] FEAT-GT-043/044 (observe sources artifact ingest + event.py helper, #514/#515) — sin PR en esta wave
- [ ] Integración Orca con Gestalt (bus :8081, replay, token filter ya mergeado) — verificar E2E
- [ ] Ollama online para insights LLM reales del thinking loop (opcional)
