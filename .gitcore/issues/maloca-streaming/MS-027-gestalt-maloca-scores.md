# [MS-027] — Export GitCore scores and worktree status to Maloca (maloca-streaming · gestalt)

| % validado | Estado | Repos | Refs | Prioridad | Esfuerzo |
|:---|:---|:---|:---|:---|:---|
| 0% | open | gestalt | | P2 | 1 day |

## Visión
Dotar a Gestalt —nuestro enrutador CLI-first en Rust que orquesta _worktrees_ aislados— de la capacidad de exportar las métricas de salud de desarrollo directamente a Xavier. De esta forma, Maloca dispondrá del estado unificado y veraz (GitCore scores y feature status) para procesarlo de forma transversal.

## Scope
- Implementar el subcomando CLI `gestalt maloca-export` dentro del ecosistema en Rust.
- Acoplar internamente las invocaciones a los comandos `gitcore-score` y `swal-verify` durante la ejecución del subcomando.
- Transformar y serializar la salida agregando _project scores_, estado actual de _features_, y salud estructural de todos los _worktrees_.
- Escribir (dump) el bloque JSON resultante directamente en la memoria local de Xavier en el bucket/path `app/gestalt/maloca/scores`.
- Soportar ejecución en demanda vía invocación manual, con parámetros preparados para su uso eventual en crons.
- Estructurar el esquema JSON de salida en compatibilidad directa con lo que espera ingerir `ScoresPage.svelte` del panel de Maloca.

## Aceptación
- [ ] El comando `gestalt maloca-export` existe y está listado en `--help`.
- [ ] Ejecutar el comando recolecta efectivamente los resultados mediante `gitcore-score` y `swal-verify`.
- [ ] Se guarda un payload JSON en la memoria local de Xavier en la ruta estipulada.
- [ ] La estructura generada incluye puntuaciones, detalles de worktree y estado funcional alineado al estándar GitCore.

## Archivos
- `src/cli/commands/maloca_export.rs`
- `src/cli/commands/mod.rs`
- `src/core/scores_dumper.rs`

## Dependencias
- `gitcore-score` CLI
- `swal-verify` CLI

## Verificación
Desde la terminal, ejecutar `cargo run -- maloca-export` o el binario compilado de `gestalt`. Posteriormente, verificar el _store_ de Xavier para corroborar que el documento en `app/gestalt/maloca/scores` exista y contenga el _dump_ con JSON parseable y esquema de puntuaciones válido.
