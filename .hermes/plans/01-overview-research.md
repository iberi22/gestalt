# Gestalt Redesign: Multi-Agent File Control + Branching + Merge

> Plan de rediseño completo para transformar Gestalt en un sistema de
> edición paralela con branching por agente, three-way merge automático,
> y sin dependencia de SurrealDB.

## Goal

Sistema agentico que permite lanzar 100+ tareas pequeñas con LLMs locales,
cada agente edita su propia branch/workspace aislado, y el sistema mergea
automáticamente los cambios. Control de versiones por archivo, manejo
inteligente de ramas, resolución automática de conflictos.

## Research Findings (Web + Delegados)

### Crate `threeway_merge` (JOYA)

https://crates.io/crates/threeway_merge  v0.1.19

**El descubrimiento clave.** Biblioteca Rust para 3-way string merging
100% compatible con Git. Statically linkea xdiff de libgit2. Soporta:

- Algoritmos: Myers, Patience, Histogram
- Merge styles: normal, diff3, zealous-diff3 (zdiff3)
- Favor modes: Ours, Theirs, Union (auto-merge sin conflict markers)
- Resultado: `content: String` + `conflicts: usize`

```rust
use threeway_merge::{merge_strings, MergeOptions,
    DiffAlgorithm, MergeStyle, MergeFavor};

let result = merge_strings(base, ours, theirs, &MergeOptions {
    algorithm: DiffAlgorithm::Histogram,
    style: MergeStyle::ZealousDiff3,
    favor: Some(MergeFavor::Union),
    ..Default::default()
})?;
// result.content -> merged text
// result.conflicts -> count of unsolvable conflicts
```

Equivalente a: `git merge-file --diff-algorithm histogram --zdiff3`

### Otras Crates

- `similar` v3.1.1 — diff library (líneas, palabras, caracteres). No
  hace merge, solo diff. Útil para diffs visuales.
- `imara-diff` v0.2.0 — Myers + Histogram, hasta 100x más rápido que
  similar en casos patológicos. Si queremos diff propio sin threeway_merge.
- `git2` (libgit2 bindings) — para `git merge-file`, `git worktree add`,
  operaciones programáticas de Git.

### Patrón Git Worktree para Agentes en Paralelo

Múltiples fuentes confirman el workflow:

```bash
# 1. Crear worktree por agente desde commit base
git worktree add /tmp/wave/agent-a <base-commit>
git worktree add /tmp/wave/agent-b <base-commit>
git worktree add /tmp/wave/agent-c <base-commit>

# 2. Cada agente trabaja en su worktree aislado
# 3. Al terminar, extraer diff de cada uno
cd /tmp/wave/agent-a && git diff <base-commit> > /tmp/patches/a.diff
cd /tmp/wave/agent-b && git diff <base-commit> > /tmp/patches/b.diff

# 4. Mergear con three-way merge o git merge
git checkout main
git merge agent-a  # o aplicar patch con threeway_merge

# 5. Limpiar
git worktree remove /tmp/wave/agent-a
```

Proyectos existentes que usan este patrón:
- Claude Code con `-w` flag (worktree nativo)
- Augment Code (Spaces con worktrees)
- Practitioners reportan hasta 371 worktrees paralelos

### Edge Cases Identificados

1. **Mismo archivo, diferentes líneas** → three-way merge funciona
2. **Mismo archivo, mismas líneas, diferentes edits** → CONFLICTO
3. **Mismo archivo, mismas líneas, mismo edit** → auto-resuelve
4. **Archivos nuevos** → siempre OK (no hay base que conflija)
5. **Archivos eliminados por un agente, editados por otro** → CONFLICTO
6. **Locks contenciosos en git** → worktrees evitan esto
   (cada worktree tiene su propio `.git/worktrees/`)

### Hallazgos Adicionales de la Delegación

#### `worktrunk` crate (Alternativa a escribir BranchManager propio)

https://crates.io/crates/worktrunk

Crate Rust con 138 versiones, 24K+ downloads, diseñado EXPRESAMENTE
para flujos de trabajo con agentes AI paralelos. Proporciona:

- CLI + library: `cargo add worktrunk`
- `wt add/list/switch/merge/remove`
- LLM commit messages
- Copy build caches entre worktrees (evita recompilar)
- `hash_port` — puerto único por worktree
- Integración Claude/Codex nativa
- Hooks pre/post merge

Podemos usar `worktrunk` como library en lugar de implementar
BranchManager desde cero, o tomar inspiración de su diseño.

#### Colocated Bare Repo Pattern

```bash
git clone --bare <url> .bare
echo "gitdir: .bare" > .git
git worktree add feature-xyz
```

Todos los worktrees son pares, no hay checkout principal.
Cada branch es igual. Ideal para waves de agentes.

#### Entity-Level Merge (Weave + tree-sitter)

Git merge a nivel de línea crea falsos conflictos cuando dos
agentes agregan funciones diferentes al mismo archivo.

**Weave** (MCP server) — entity-aware merge driver usando tree-sitter:
- Resuelve 31/31 escenarios vs git's 15/31
- Propuesto como integración para Codex CLI
- Para nuestro caso: descomposición entity-aware ("agente A
  es dueño de funciones X,Y,Z") hace conflictos estructuralmente
  imposibles

#### Integraciones Nativas

| Proyecto | Comando |
|----------|---------|
| Claude Code | `claude --worktree feature-auth` |
| Worktrunk | `wt switch -x claude -c <branch>` |
| Claude + worktrees | `claude -w feature-auth` |

### Proyectos Open Source Relevantes

| Proyecto | Approach | Merge Strategy |
|----------|----------|----------------|
| Aider | Git-commits automáticos, /undo | Serial (no paralelo) |
| SWE-agent | Agent-Computer Interface | Task secuencial |
| OpenHands | Multi-agent con delegación | Docker sandbox |
| Claude Code | `-w` flag para worktrees | Git merge manual |
| **Gestalt (propuesto)** | Branch per agent + MergeEngine | Three-way auto + LLM |
