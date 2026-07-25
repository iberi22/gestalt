# Conflict Prevention & Merge Estrategia Final

Basado en investigación web + pruebas reales.

## Hallazgo #1: Weave — Entity-Level Merge Driver

https://github.com/ataraxy-labs/weave
`cargo install weave` · `brew install weave`

**Creado por Ataraxy Labs, endorsado por Elijah Newren** (autor del
merge-ort de Git, el algoritmo default desde Git 2.34).

### Qué hace

Reemplaza el merge de Git a nivel de **líneas** con merge a nivel de
**entidades semánticas** (funciones, clases, structs, JSON keys).

```
Git line-level:                  Weave entity-level:
                                 
@@ -195,5 +195,8 @@              fn validate_token()
-foo                              fn validate_token()
+bar                              fn parse_config()     ← nueva
+baz                              fn validate_schema()  ← nueva
+qux                              (sin conflicto)
+quux
```

### Resultados

| Escenario | Git (line-level) | Weave (entity-level) |
|-----------|-----------------|---------------------|
| 2 agentes agregan diferentes funciones al mismo archivo | ❌ CONFLICTO (líneas se solapan) | ✅ AUTO-MERGE (entidades diferentes) |
| 2 agentes modifican la MISMA función | ❌ CONFLICTO | ❌ CONFLICTO (correcto) |
| 1 agente modifica, 1 agente elimina | ❌ CONFLICTO confuso | ✅ Mensaje claro: "function X modified in ours, deleted in theirs" |
| 31 escenarios reales | ✅ 15/31 resueltos | ✅ 31/31 resueltos |

Zero regresiones validado en git/git, CPython, Flask, TypeScript compiler.

### Lenguajes Soportados (28 + config)

Rust, Python, TypeScript, JavaScript, Go, Java, C, C++, C#, Ruby,
Kotlin, Swift, Bash, plus JSON, YAML, TOML, Markdown.
Fallback a line-level para no-soportados.

### MCP Server para Prevención de Conflictos

```
weave_claim_entity("UserAuthService.validate_token")  ← agente A reclama
weave_who_is_ing("file.rs")                     ← agente B consulta
weave_preview_merge("origin/main")          ← simular merge antes
```

**Flujo:** Agente A reclama función → Agente B consulta → ve que está
ocupada → elige otra función → **cero conflictos**.

## Hallazgo #2: El "Merge Tax" (Dave Paola)

### El problema matemático

```
N agentes en paralelo → conflictos potenciales = N*(N-1)/2
                                                    
2 agentes → 1 conflicto potencial    (fácil)
4 agentes → 6 conflictos potenciales (manejable)
9 agentes → 36 conflictos potenciales (desastre)
```

Cada merge cambia la base para los demás branches, creando nuevos
conflictos en cascada. El overhead crece **cuadráticamente** con N.

### La solución: Sequential Merges

```
❌ Malo: mergear todo al final
   Branch A ──┐
   Branch B ──┤── merge A+B+C (3 vías = muchos conflictos)
   Branch C ──┘

✅ Bueno: mergear secuencialmente
   Branch A ──→ merge A → main actualizado
   Branch B ──→ rebase B sobre main → merge B
   Branch C ──→ rebase C sobre main → merge C
```

Cada merge actualiza main. Los branches restantes se rebasean
contra el nuevo main. El árbol de conflictos es lineal, no
combinatorio.

## Estrategia Final de Prevención de Conflictos

### Nivel 1: File-Island Pattern (más importante)

```
REGLAS ESTRICTAS:
- 1 issue = 1 archivo específico
- Dos issues NUNCA tocan el mismo archivo en paralelo
- Si tocan el mismo archivo → van SECUENCIALES en la misma wave
```

Si respetas esto → **conflictos = 0**. Cada agente edita su propio
archivo, no hay solapamiento posible.

### Nivel 2: Entity-Awareness via Weave

Cuando dos issues inevitablemente tocan el mismo archivo (ej: ambos
agregan funciones a `mod.rs`), Weave mergea a nivel de entidad:

```
Agente A agrega fn parse_config() al archivo
Agente B agrega fn validate_schema() al mismo archivo
Weave: entidades diferentes → auto-merge, 0 conflictos
```

### Nivel 3: Weave MCP Coordination

Para waves grandes donde múltiples agentes trabajan en el mismo
archivo:

```
1. Cada agente reclama entidades vía Weave MCP
2. Si la entidad ya está reclamada → elige otra
3. Si no hay Weave → git merge driver entity-level
```

### Nivel 4: Sequential Merges

```
NO mergear todos los branches a la vez.

FLUJO CORRECTO:
1. Merge branch A → main
2. Rebase branch B sobre main actualizado
3. Merge branch B → main
4. Rebase branch C sobre main actualizado
5. Merge branch C → main
...
```

Esto evita el merge tax cuadrático.

## Límite de Concurrencia

```
┌──────────────┬──────────┬──────────────┐
│ Agentes      │ Conflic- │ Recomenda-   │
│ simultáneos  │ tos      │ ción         │
├──────────────┼──────────┼──────────────┤
│ 2-4          │ Bajos    │ ✅ Ideal     │
│ 5-8          │ Medios   │ ⚠️ Con Weave │
│ 9-15         │ Altos    │ ❌ No        │
│ 16+          │ Extremo  │ 🚫 Evitar    │
└──────────────┴──────────┴──────────────┘
```

**Límite práctico:** 4-8 agentes simultáneos.
100+ tareas encoladas, 4-8 ejecutándose.

## Herramientas Finales para el Stack

| Herramienta | Propósito | Instalación |
|-------------|-----------|-------------|
| `git worktree` | Aislamiento por agente | Built-in en git |
| `weave` (cargo) | Entity-level merge driver | `cargo install weave` |
| `weave setup` | Configurar .gitattributes | `weave setup` en el repo |
| `weave-mcp` | Entity coordination | `cargo install weave-mcp` |
| `threeway_merge` crate | Fallback line-level merge | `cargo add threeway_merge` |
| `agy` / `kimi` / `claude` | CLI agents | Ya instalados |

## Cómo Cambia la Arquitectura

```
ANTES (plan original):
  MergeEngine.three_way_merge(base, ours, theirs)
  → solo line-level merge, conflictos frecuentes

DESPUÉS (con Weave):
  .gitattributes → weave merge driver
  git merge → entity-level merge automático
  Weave MCP → coordinación preventiva de entidades
  Sequential merges → sin merge tax

  SOLO para casos donde Weave no funciona:
  MergeEngine.three_way_merge() como fallback
```
