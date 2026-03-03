# Agent Benchmark

CLI para evaluar agentes IA y guardar historico en SQLite.

## Comandos

```bash
python -m agent_benchmark list-tasks
python -m agent_benchmark run --task code_sum_001 --agent codex
python -m agent_benchmark leaderboard --task code_sum_001
python -m agent_benchmark latest --limit 10
python -m agent_benchmark sync-rust
```

## Integracion real con agentes

Por defecto, `run` usa modo simulado si no encuentra comando para el agente.

Para ejecutar un agente real:

```bash
python -m agent_benchmark run \
  --task code_api_001 \
  --agent codex \
  --command "codex run --prompt {prompt}"
```

Tambien puedes definir variable de entorno por agente:

```bash
set AGENT_BENCHMARK_CODEX_CMD=codex run --prompt {prompt}
set AGENT_BENCHMARK_GEMINI_CMD=gemini --prompt {prompt}
```

## Formula de score

```
SCORE = (correctness * 0.4) +
        (efficiency * 0.2) +
        (readability * 0.2) +
        (success_rate * 0.2)
```

## Base de datos

Ruta por defecto:

`benchmarks/agent_benchmark.sqlite`

