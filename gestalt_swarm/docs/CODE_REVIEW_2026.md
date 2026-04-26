# Gestalt Swarm — Code Review: WARM-AGENTS Epic

**Date:** 2026-04-26
**Reviewer:** Jean (subagent)
**Issues reviewed:** #286, #287, #288

---

## Aggregate Summary

Issues #286, #287, #288 comparten el mismo objetivo: implementar WARM-AGENTS — agentes pre-inicializados con pool reuse, health monitoring, y feedback loop de metricas.

**Patron identificado:** Los 3 issues son facets de un mismo epic que requiere un `SwarmState` centralizado. Los modulos actuales (`pool.rs`, `health.rs`, `shared.rs`) fueron escritos en paralelo pero nunca integrados entre si ni con `main.rs`.

---

## Hallazgos por Issue

### Issue #286 — Agent Pool Reuse (#286)

| Feature | Estado | Comment |
|---------|--------|---------|
| Agent pool lifecycle (pre-warm, idle timeout, max pool) | ⚠️ Partial | `pool.rs` existe pero tiene BUG de re-indexacion en evict() |
| Session affinity | ❌ Missing | No existe concepto de session/task-group ID |
| Health check pings | ⚠️ Partial | Heartbeat existe pero hace leak de tasks |
| Pool hit rate vs cold-start metrics | ❌ Missing | `PoolStats.hit_rate` y `cold_start_rate` no se calculan |

### Issue #287 — Health Monitoring (#287)

| Feature | Estado | Comment |
|---------|--------|---------|
| Heartbeat mechanism | ⚠️ Partial | Existe pero hace leak de tasks |
| Auto-restart on timeout/failure | ❌ Missing | `RecoveryManager` es dead code, nunca se instancia |
| Dead agent graceful removal | ⚠️ Partial | `unregister_agent()` marca como Dead pero no limpia |
| Alert on repeated failures | ❌ Missing | Eventos se emitern por watch channel pero nadie suscribe |

### Issue #288 — Feedback Loop (#288)

| Feature | Estado | Comment |
|---------|--------|---------|
| Execution metrics (latency, success, token usage) | ⚠️ Partial | Solo duration_ms y success — tokens no se capturan |
| Aggregate metrics report | ❌ Missing | No hay generacion de report post-batch |
| Auto-generate GitHub issue on drops | ❌ Missing | No existe logica de deteccion de performance drops |
| Track ROI over time | ❌ Missing | No hay persistencia de metricas historicas |

---

## Technical Debt Adicional (Issue #289)

Ver: https://github.com/iberi22/gestalt/issues/289

### Critical (2)
1. Heartbeat tasks leak on agent completion (`main.rs`)
2. `PooledAgentGuard` Drop impl es unsafe en contexto async (`pool.rs`)

### High (4)
3. `checkout()` retorna `Option` en vez de esperar — pool vacio no bloquea (`pool.rs`)
4. Session affinity no esta implementada
5. `evict()` re-indexacion no implementada — BUG post-evict (`pool.rs`)
6. `RecoveryManager` es dead code — nunca se instancia (`health.rs`)

### Medium (3)
7. `should_restart()` siempre retorna `true` (`health.rs`)
8. `run_agent()` construye LLM provider por cada ejecucion (`main.rs`)
9. Metricas del issue #288 no existen

### Low (3)
10. `HealthChecker` lock re-adquisition inconsistency (`health.rs`)
11. `AgentPool` no tiene tests
12. No hay integracion entre modulos

---

## Recomendaciones de Arquitectura

### 1. Crear `SwarmState` centralizado

```rust
pub struct SwarmState {
    pub pool: AgentPool,
    pub monitor: SwarmHealthMonitor,
    pub metrics: SwarmMetrics,
    pub shutdown: CancellationToken,
}
```

Todos los modulos deberian recibir referencia a `SwarmState` en vez de crear sus propios `Arc<RwLock>`.

### 2. Integrar `pool.rs` con `health.rs`

El pool de agentes necesita preguntar al health monitor antes de hacer checkout. Agentes unhealthy no deberian ser devueltos al pool.

### 3. Usar `CancellationToken` para shutdown

Reemplazar `shutdown_flag: AtomicBool` con `CancellationToken` de tokio_util para propagar cancellation a todos los tasks hijos.

### 4. Escribir tests antes de implementar features

El issue #289 identifica 0 tests en gestalt_swarm/src. Cada modulo tiene logica compleja (pool eviction, health state machine, recovery logic) que necesita tests.

---

## Files Revisados

- `gestalt_swarm/src/main.rs`
- `gestalt_swarm/src/pool.rs`
- `gestalt_swarm/src/health.rs`
- `gestalt_swarm/src/shared.rs`
