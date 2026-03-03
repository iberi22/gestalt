---
github_issue: 82
title: "[FEATURE] Sistema de Benchmark y Leaderboard para Agentes IA"
labels:
  - feature
  - benchmark
assignees: []
status: closed
closed_on: 2026-03-03
last_reviewed: 2026-03-03
---

## Objective
Implementar un sistema desacoplado de benchmark para agentes IA con leaderboard histórico.

## Scope
- `agent_benchmark/` (nuevo módulo Python).
- `benchmarks/tasks/` (suite de tareas).
- Integración con benchmarks existentes del monorepo.

## Acceptance
- [x] Runner agnóstico al agente.
- [x] 7+ tareas de benchmark.
- [x] Persistencia SQLite histórica.
- [x] API/CLI para leaderboard.
- [x] Cálculo de score compuesto.
