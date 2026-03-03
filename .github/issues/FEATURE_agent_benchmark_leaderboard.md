---
github_issue: 82
title: "[FEATURE] Sistema de Benchmark y Leaderboard para Agentes IA"
labels:
  - feature
  - benchmark
assignees: []
status: open
last_reviewed: 2026-03-03
---

## Objective
Implementar un sistema desacoplado de benchmark para agentes IA con leaderboard histórico.

## Scope
- `agent_benchmark/` (nuevo módulo Python).
- `benchmarks/tasks/` (suite de tareas).
- Integración con benchmarks existentes del monorepo.

## Acceptance
- [ ] Runner agnóstico al agente.
- [ ] 7+ tareas de benchmark.
- [ ] Persistencia SQLite histórica.
- [ ] API/CLI para leaderboard.
- [ ] Cálculo de score compuesto.
