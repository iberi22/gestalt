# SWAL Architecture: Gestalt & Xavier Pipeline

## Central Router vs Leaf TUI Model (1 Window → N Repos)

To scale multi-agent execution across 5+ active repositories without context pollution and log truncation, Gestalt separates central orchestration from per-project telemetry using a 1 Router + N Leaf TUIs pattern.

### Architecture Overview

```
                          ┌──────────────────────────┐
                          │    Router Central        │
                          │   (1 Hermes Window)      │
                          │                          │
                          │ - gh issue list          │
                          │ - xavier recall harvest  │
                          │ - No code modification   │
                          └─────────────┬────────────┘
                                        │
                      Emits Bus Events (project tagged)
                                        │
             ┌──────────────────────────┴──────────────────────────┐
             ▼                                                     ▼
┌──────────────────────────┐                             ┌──────────────────────────┐
│   Gestalt Event Bus      │                             │   Gestalt Event Bus      │
│   (StateDb / :8081)      │                             │   (StateDb / :8081)      │
└────────────┬─────────────┘                             └────────────┬─────────────┘
             │                                                        │
             ▼                                                        ▼
┌───────────────────────────────────────────────────────────────────────────────────┐
│                           tmux 4-Pane Dashboard                                   │
│                         (`periferia/gestalt-tui.sh`)                              │
│                                                                                   │
│  ┌─────────────────────────────────────┬───────────────────────────────────────┐  │
│  │ Pane 0: gara-g                      │ Pane 1: hosteler-ia                   │  │
│  │ `gestalt bus replay --project gara-g`│ `gestalt bus replay --project hosteler-ia`│  │
│  ├─────────────────────────────────────┼───────────────────────────────────────┤  │
│  │ Pane 2: xavier                      │ Pane 3: OrionHealth                   │  │
│  │ `gestalt bus replay --project xavier`│ `gestalt bus replay --project OrionHealth`│  │
│  └─────────────────────────────────────┴───────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────────────────┘
```

### Components

1. **Router Central (1 Window)**:
   - Operating environment for Hermes / Orchestrator.
   - Responsible only for high-level triage, `gh issue list`, and `xavier recall` harvesting.
   - Does not perform code execution or tailing directly in the router window.

2. **Leaf TUIs (4 Panes via `periferia/gestalt-tui.sh`)**:
   - Isolates logs, event streams, and PR statuses per project repository (`gara-g`, `hosteler-ia`, `xavier`, `OrionHealth`).
   - Uses `gestalt bus replay --since 1h --project <repo> --json` to filter event bus events by `payload.project`.
   - Prevents log interleaving and provides clean visual monitoring across multiple concurrent sub-agent executions.
