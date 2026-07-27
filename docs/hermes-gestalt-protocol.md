# Protocolo Hermes ↔ Gestalt v1

## Arquitectura

```
HERMES (orchestrator)
  │ POST /v1/orchestrate
  ▼
GESTALT (technical backend)
  ├── VFS isolation
  ├── Block editing
  ├── Conflict detection
  ├── Parallel execution
  └── Xavier sync
```

## Contrato

### OrchestrationRequest (Hermes → Gestalt)

```json
{
  "task": "edit main.rs",
  "agents": [{ "name": "agy", "command": "agy edit ..." }],
  "context": { "xavier_results": [...], "git": {...} },
  "parallelism": 1
}
```

### OrchestrationResult (Gestalt → Hermes)

```json
{
  "run_id": "01ABCD1234",
  "agent_results": [...],
  "edits": [{ "path": "src/main.rs", "op": "Replace", ... }],
  "conflicts": []
}
```

## Implementación actual

Hoy: `gestalt xavier cycle "task" --agent "cmd"`
Futuro: `gestalt xavier orchestrate --request request.json`
