# gestalt-swarm Skill

## Descripción
Gestalt Swarm — Parallel execution bridge for multi-agent orchestration. Dispatch N agents concurrently for high-speed automation.

## Uso

```bash
# Build
cargo build --release -p gestalt_swarm

# Run
cargo run --release -p gestalt_swarm -- --agents 4
```

## Arquitectura

```
SwarmCoordinator
├── TaskQueue (priority queue)
├── AgentRegistry
└── HealthMonitor

Agent implementations:
├── CliAgent (shell commands)
└── LlmAgent (LLM-powered)
```

## Configuración

```bash
# Agents count
--agents 4

# Timeout
--timeout 300

# Swarm mode in gestalt binary
cargo run -p gestalt_timeline --bin gestalt -- --swarm
```

## Agentes Disponibles

Los agentes se registran en `AgentRegistry`. Cada agente implementa el trait `Agent` con `execute(task) -> Result<Value>`.

## Estado

✅ Operativo — SwarmCoordinator 95% implementado

---

*Gestalt Swarm — Parallel AI agents that actually execute.*
