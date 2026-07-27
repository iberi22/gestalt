# ISSUE GENERATION POSTMORTEM — Casos #386 y #389

> Actualizado: 2026-07-27
> Skill fix: `issue-generation-with-verification`

## Caso 1: Issue #386 — Checkpointer "no existe"

**Síntoma:** Jules falló porque el issue decía "checkpoint.rs NO existe"
**Realidad:** checkpoint.rs EXISTE con 304 líneas + 249 líneas de tests + 12 tests
**Causa Raíz:** El generador no ejecutó `ls -la`, `wc -l`, ni `grep tests` antes de crear el issue
**Fix:** Skill actualizado con verificación de codebase OBLIGATORIA antes de crear issues

### Lecciones
- Siempre verificar existencia de archivos con `ls` + `wc -l`
- Siempre verificar tests existentes con `grep -c "#[test]" tests/*`
- Incluir commit hash actual en el issue

## Caso 2: Issue #389 — Documentación "15 entregables"

**Síntoma:** Jules falló porque el issue pedía 15 entregables en una sola ejecución
**Realidad:** docs/guides/ no existe, ARCHITECTURE.md ya existe en raíz, WALKTHROUGH.md ya cubre tutorial
**Causa Raíz:** 
- No verificó qué documentación YA existe antes de asumir que no existe
- Pidió 6 archivos nuevos + diagramas Mermaid + ADRs = ~15 entregables
- Sin priorización (P0, P1, P2)

### Lecciones
- Máximo ~5 entregables por issue (dividir en sub-issues si excede)
- Verificar directorios existen (`docs/guides/` no existe → problema)
- Si ARCHITECTURE.md ya existe → decir "Actualizar" no "Crear"
- Priorizar: P0 > P1 > P2
- Archivos existentes que ya cubren parte del tema: WALKTHROUGH.md, QUICKSTART.md, ARCHITECTURE.md

## Patrón Común

Ambos casos comparten la MISMA causa raíz:
**El generador de issues NO verificó el estado actual del codebase.**

El fix es el mismo: el skill `issue-generation-with-verification` implementa
un workflow de 4 pasos que previene ambos errores.

## Referencias en Xavier

- `gestalt/postmortem/issue-386-checkpointer` — Postmortem del Caso 1
- `~/.hermes/skills/gestalt/issue-generation-with-verification/SKILL.md` — Skill parcheado
