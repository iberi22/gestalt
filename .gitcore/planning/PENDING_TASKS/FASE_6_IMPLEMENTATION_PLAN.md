# Fase 6 - Plan de Implementacion (Resiliencia y Aislamiento)

Fecha: 2026-02-23
Estado: En preparacion
Referencia: F6-01 a F6-07 en `.gitcore/planning/TASK.md`

## Analisis inicial de `gestalt_timeline/src/services/runtime.rs`

Hallazgos relevantes:
- `AgentRuntime` depende de `max_steps: usize` y corta el loop en un limite fijo (actualmente 20).
- `OrchestrationAction::ReadFile` y `OrchestrationAction::WriteFile` usan `tokio::fs` directamente.
- El estado persistido usa `max_steps` en `AgentRuntimeState::new(...)`, lo que asume ejecucion finita.
- No existe rutina de compaction del historial, solo `history_tail` truncado para persistencia.

Impacto:
- No cumple Decision 5 (VFS Overlay) por escritura directa a disco.
- No cumple Decision 6 (Elastic Loops) por tope fijo de pasos.
- No existe base de supervision tipo Hive (Decision 7) en este modulo.

## Primer paso tecnico propuesto (inicio de implementacion)

1. Implementar `VirtualFs` como adaptador e integrar su uso en `AgentRuntime` para `ReadFile` y `WriteFile` (F6-01 + base de F6-02).

Por que este paso primero:
- Es el requisito de seguridad mas critico: evita modificar disco antes de validacion/flush.
- Reduce riesgo al encapsular IO sin romper aun el motor de decisiones.
- Habilita despues un `flush` transaccional bajo supervisor.

## Alcance exacto del Paso 1

- Crear `gestalt_timeline/src/services/vfs.rs` con:
  - Estructura `VirtualFs` (read-through + write-in-memory).
  - Metodos async minimos: `read_to_string`, `write`, `create_dir_all`, `flush`.
  - Mapa en memoria para overlays por `path`.
- Inyectar `VirtualFs` en `AgentRuntime` (campo nuevo y constructor `new` actualizado).
- Reemplazar llamadas directas a `tokio::fs` en `execute_action`:
  - `ReadFile` -> `self.vfs.read_to_string(...)`
  - `WriteFile` -> `self.vfs.write(...)`
- Mantener persistencia fisica bloqueada por defecto:
  - `WriteFile` solo modifica overlay.
  - `flush` no se invoca desde `run_loop` aun (se deja para control de supervisor).

## Criterios de aceptacion del Paso 1

- `WriteFile` no altera disco fisico hasta `flush`.
- `ReadFile` devuelve contenido overlay si existe; si no, lee del disco.
- Tests unitarios nuevos para VFS:
  - Lectura read-through.
  - Escritura aislada en memoria.
  - Persistencia diferida via `flush`.
- Compilacion y tests del crate `gestalt_timeline` en verde.

## Orden sugerido posterior (sin implementar aun)

1. F6-03: Motor de compaction de contexto.
2. F6-04: `run_loop` elastico sin `max_steps` fijo.
3. F6-05: Migracion a Hive.
4. F6-06/F6-07: locking e integrador reviewer/merge.
