# 🚀 Quick Start — Gestalt Swarm

> **Tiempo estimado: 5 minutos**

Gestalt Swarm te permite ejecutar múltiples agentes CLI en paralelo para automatización masiva. Sin configuración compleja.

## Prerequisite

Python 3.8+ y ripgrep (opcional pero recomendado):

```bash
# Windows (via chocolatey)
choco install ripgrep

# macOS
brew install ripgrep

# Linux
apt install ripgrep
```

## Step 1 — Clone & Run

```bash
# Clone el repo
git clone https://github.com/iberi22/gestalt-rust.git
cd gestalt-rust

# Ejecuta tu primer swarm
python swarm_bridge.py --goal "git status" --agents "git_analyzer,git_status" --json --quiet
```

**Output esperado:**
```
✅ git_analyzer: success (63ms)
✅ git_status: success (86ms)
```

## Step 2 — Smart Selection (Sin Especificar Agentes)

```bash
# El swarm elige los agentes automáticamente
python swarm_bridge.py --goal "security audit of codebase" --json
```

**Output:**
```
🐝 Goal: security audit of codebase
📋 Selected agents (2): security_audit, find_todos
✅ security_audit: success (156ms)
✅ find_todos: success (89ms)
```

## Step 3 — Preview con Dry Run

```bash
# Ve qué agentes se seleccionarían sin ejecutar
python swarm_bridge.py --goal "comprehensive codebase analysis" --dry-run
```

**Output:**
```
🐝 Goal: comprehensive codebase analysis
📋 Selected agents (5): code_analyzer, file_scanner, security_audit, find_todos, metrics
⏭️  Dry run — no agents executed
```

## Step 4 — Parallel Execution Benchmark

```bash
# 3 agentes en paralelo vs secuencial
time python swarm_bridge.py --goal "quick check" --agents "git_analyzer,git_status,env_check" --json --quiet
```

**Tiempo típico:** ~91ms (vs ~300ms secuencial)

## Step 5 — Integrate with OpenClaw

En tu sesión de OpenClaw, simplemente escribe:

```
analiza gestalt-rust en busca de errores de seguridad
```

El LLM detectará que necesitas Gestalt Swarm y ejecutará:
```bash
python swarm_bridge.py --goal "security audit of gestalt-rust" --max-agents 10 --json
```

## Options Reference

| Flag | Description | Ejemplo |
|------|-------------|---------|
| `--goal "..."` | Descripción del objetivo | `--goal "find todos"` |
| `--agents "a,b"` | Agentes específicos | `--agents "find_todos,security_audit"` |
| `--max-agents N` | Máximo de agentes | `--max-agents 20` |
| `--dry-run` | Preview sin ejecutar | `--dry-run` |
| `--json` | Output JSON | `--json` |
| `--watch` | Resultados parciales | `--watch --timeout 30` |

## Agentes Disponibles

```bash
# Todos los agentes
python swarm_bridge.py --goal "check deps" --agents "dep_check,cargo_check,git_status" --json

# Análisis rápido
python swarm_bridge.py --goal "quick status" --agents "git_analyzer,git_status" --json --quiet
```

## Troubleshooting

**"Command not found: rg"**
```bash
choco install ripgrep  # Windows
brew install ripgrep  # macOS
```

**"Timeout after Xs"**
```bash
# Aumenta el timeout por agente
python swarm_bridge.py --goal "..." --agents "metrics" --json
# O usa streaming con --watch para no esperar
```

## Siguiente Paso

- Lee [README.md](README.md) para documentación completa
- Explora `.gitcore/ARCHITECTURE.md` para entender el diseño
- Abre issues en GitHub si tienes preguntas

---

**¿Problemas?** https://github.com/iberi22/gestalt-rust/issues
