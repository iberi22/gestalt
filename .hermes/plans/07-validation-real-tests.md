# Validación: ¿Cómo funciona Gestalt con CLI Agents?

## Prueba Real Realizada (Jul 22, 2026)

### Setup
```bash
git init /tmp/gestalt-test
echo "test content" > test.txt
git commit -m "init"

# Crear worktree con branch propia
git worktree add -b agent-b-home /home/belal/gestalt-agent-b main
```

### Resultado: agy funciona ✅

```bash
agy -p "Edit test.txt: change 'test content' to 'modified by agy'"
     --model gemini-3.6-flash-medium
     --effort medium
     --add-dir /home/belal/gestalt-agent-b
     --dangerously-skip-permissions
```

**Archivos modificados exitosamente por agy en el worktree:**
- `test.txt` → contenido cambiado
- `result.py` → archivo nuevo creado con función hello_world()

### Resultado: kimi funciona (con configuración) ✅

```bash
kimi -p "Read file in workspace..."
     --add-dir /home/belal/gestalt-agent-b
     --model kimi-code/k3
```

Requiere que el modelo esté configurado en `~/.kimi-code/config.toml`.

---

## Mecanismo de Integración: Cómo Funciona

Gestalt NO se integra con los agentes a nivel de API o plugin.
Es un **orquestador CLI** que:

```
1. Crea worktree → git worktree add -b agent-N /ruta/worktree main
2. Asigna tarea → agy -p "fix: cambia X por Y en archivo.rs"
                     --add-dir /ruta/worktree
3. Agente edita → usa sus tools (read_file, write_file, edit)
                  en el worktree que le pasamos
4. Gestalt detecta → git diff main HEAD dentro del worktree
5. Gestalt mergea → threeway_merge::merge_strings(base, ours, theirs)
```

**El agente NO necesita saber que Gestalt existe.** Solo necesita:
- Un directorio donde trabajar (worktree)
- Una tarea que hacer (prompt)
- Capacidad de leer/escribir archivos (todos los CLI agents la tienen)

### Condiciones para que funcione

| Condición | Estado | Notas |
|-----------|--------|-------|
| CLI agent acepta `--add-dir` | ✅ agy, kimi, claude lo tienen | codex no instalado, jules es VM |
| CLI agent puede editar archivos | ✅ agy sí, kimi vía `--auto` | codex/claude también |
| Worktree visible desde sandbox | ⚠️ NO en `/tmp/` | ✅ En `~/` funciona |
| CLI agent tool execution | ✅ agy con `--dangerously-skip-permissions` | kimi necesita `--auto` |
| Detectar qué cambió | ✅ `git diff` post-ejecución | Funciona siempre |

### ⚠️ Limitación: Bubblewrap Sandbox

agy corre dentro de **bubblewrap** (sandbox) por defecto.
Esto significa que **NO puede acceder a `/tmp/`**.

**Solución:** Crear worktrees dentro de `~/` o un directorio visible
para la sandbox.

```bash
# ❌ No funciona (sandbox no ve /tmp)
git worktree add /tmp/wave/agent-a main
agy --add-dir /tmp/wave/agent-a ...  # Error: bwrap: path not found

# ✅ Funciona (sandbox ve ~/)
git worktree add ~/gestalt-worktrees/wave-N/agent-a main
agy --add-dir ~/gestalt-worktrees/wave-N/agent-a ...  # ✅
```

### ¿Qué agentes funcionan?

| Agente | `--add-dir` | Edita archivos | Sandbox | Veredicto |
|--------|-------------|----------------|---------|-----------|
| **agy** | ✅ | ✅ | bwrap (default) | ✅ **Recomendado** |
| **kimi** | ✅ | ✅ (con `--auto`) | no sandbox | ✅ (configurar modelo) |
| **claude** | ✅ `--add-dir` | ✅ | no sandbox | ✅ (nativo worktree) |
| **codex** | ❓ (no instalado) | ✅ | sandbox | ⚠️ Probar |
| **jules** | ❌ (VM-based) | ✅ (en VM) | VM completa | ❌ No sirve para local |

---

## Validación del Plan: ¿Es Realista?

### ✅ Lo que es REAL y funciona hoy

**1. Git worktree + agy = edición en paralelo**
Probado. Crea worktree, agy modifica archivos, git diff detecta cambios.
Patrón usado por Claude Code, Worktrunk, y practitioners con 4+ agentes.

**2. CLI agents tienen `--add-dir`**
agy ✅, kimi ✅, claude ✅ — los tres aceptan directorio extra de trabajo.

**3. `git diff` post-ejecución para detectar cambios**
Funciona siempre, independiente del agente. No necesita parsear output.

**4. `git worktree add -b agent-N main`** para aislamiento
Cada agente tiene su propia branch + directorio. Cero contención.

### ⚠️ Lo que tiene riesgos (pero es factible)

**1. Eliminar SurrealDB (Fase 1)**
Riesgo: BAJO. Es código muerto. FileManager + VFS no dependen de ella.
Los servicios (timeline, project, task) son wrappers que se pueden eliminar.
~20 archivos, ~2h de trabajo mecánico.

**2. Three-way merge con `threeway_merge` crate**
Riesgo: BAJO. El crate existe, tiene 20 versiones, 576+ tests.
API limpia: `merge_strings(base, ours, theirs, options)`.
Solo hay que decidir: `favor: Union` vs detectar conflictos.

**3. BranchManager (worktree management)**
Riesgo: BAJO. Son wrappers de `std::process::Command("git worktree ...")`.
~200 líneas. Alternativa: `worktrunk` crate (138 versiones, diseñado para esto).

**4. AgentRunner (spawn agents)**
Riesgo: MEDIO. El problema real no es el spawn — es que los agentes:
- A veces no terminan (timeout)
- A veces producen código que no compila
- A veces alucinan rutas de archivos
- Usan diferentes formatos de edición (SEARCH/REPLACE vs write entire file)

**Solución:** No depender del formato de output del agente.
`git diff` después de que termine → detecta cambios reales.
`cargo check` después de mergear → valida compilación.

### ❌ Lo que NO es realista sin ajustes

**1. "100+ tareas en paralelo con LLMs locales"**
NO es simultáneo. 100+ tareas en COLA con 4-8 ejecutándose a la vez.
Razones:
- GPU VRAM limitada (~24GB = 4-8 modelos pequeños)
- API rate limits (si usas cloud)
- Contención de disco (todos escribiendo al mismo repo)

**Realista:** Cola de 100+ tareas, 4-8 agentes simultáneos, 
bounded concurrency pool.

**2. "LLMs pequeños para todo"**
Para tareas triviales (fix typo, rename variable) → sí.
Para merge conflict resolution → necesita modelo más grande.
Para tareas que requieren entender contexto → depende.

**Realista:** Modelo chico para ediciones simples (Qwen2.5-Coder 7B,
DeepSeek-Coder 6.7B). Modelo grande solo para merge conflicts
o tareas complejas.

**3. Timeline de 12-15h para todo el proyecto**
Optimista. Para un prototipo funcional → sí (12-15h).
Para producción (edge cases, error handling, tests) → 40-60h.

**Realista:**
- Fase 1 (eliminar SurrealDB): 2-3h ✅
- Fase 2a (BranchManager básico): 1-2h ✅
- Fase 2b (MergeEngine básico): 2-3h ✅
- Fase 3a (AgentRunner + CLI): 3-4h ✅
- **Prototipo funcional: ~10-12h**
- Tests + edge cases + polish: +20-30h

---

## Conclusión de Validación

### ✅ El plan es REALISTA con estos ajustes:

1. **BranchManager** → Simple wrapper de `git worktree` + `git diff`
2. **MergeEngine** → `threeway_merge` crate con `favor: Union`
3. **AgentRunner** → Shell out a agy/kimi/claude con `--add-dir`
4. **Concurrencia** → 4-8 simultáneos, 100+ en cola
5. **Worktrees** → En `~/gestalt-worktrees/` (no /tmp/) por sandbox
6. **Detección de cambios** → `git diff` post-ejecución
7. **Validación** → `cargo check` post-merge

### ❌ Lo que NO es realista sin ajustes:
- 100+ tareas estrictamente simultáneas (batch de 4-8)
- Todo con LLMs pequeños (merge conflicts necesitan más)
- 12-15h total (sí para prototipo, no para producción)

### 🔥 Arquitectura Simplificada (realista)

```
gestalt wave init --id wave-07 --tasks tasks.json
  → git worktree add por cada tarea

gestalt wave run --id wave-07 --concurrency 4
  → spawn agy/kimi/claude en cada worktree (máx 4 simultáneos)
  → collect diffs via git diff

gestalt wave merge --id wave-07
  → threeway_merge para archivos con múltiples edits
  → files sin conflicto se aplican directo

gestalt wave verify --id wave-07
  → cargo check && cargo clippy

gestalt wave finalize --id wave-07
  → git commit + cleanup worktrees
```
