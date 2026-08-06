# GESTALT — Informe de Implementación Real, Gaps y Plan (2026-08-06)

## 1. % DE IMPLEMENTACIÓN REAL

| Métrica | Valor | Fuente |
|---------|-------|--------|
| Features declaradas | 15 | .gitcore/features.json |
| Claimed promedio | **85.7%** | features.json |
| Real (escáner 7-checks) | **41.0%** | xavier verify features |
| Gap promedio | 44.7% | escáner |
| Trazabilidad (paths+REQ) | **15/15 PASS** | verify-pipeline.sh |
| Tests unitarios pasando | **119** | cargo test (core 69, router 23, state 20, ws 7) |
| Compilación workspace | ✅ OK | cargo check --workspace (excl. gestalt_swarm) |

### Features con gap real (no solo formato)
- **feat-belief-graph: 0%** — código ausente (el PR #503 añadió persistencia JSON, pero el graph core no existe)
- **feat-hybrid-search: 35%** — gestalt-search (Tantivy BM25) existe y compila, pero sin tests documentados ni integración completa
- **feat-mcp-server: 70%** — MCP server funcional (18+ tools, tests de integración #496) pero sin BM25 tools aún (PR #504 ya integrado)
- **12 features al 100% claimed sin tests documentados** → inflación por formato (usa `evidence` en vez de `implemented_in`/`tests`/`status` canónicos)

### Código real por crate (LOC)
core 8,235 · router 6,901 · cli 3,745 · mcp 1,691 · state 1,971 · search 930 · synapse-agentic 955 · ws 433 · merge 1 (vestigio)

## 2. GAPS CRÍTICOS (hallazgos del análisis Kimi + auditoría)

### 🔴 Seguridad (CRÍTICO — resuelto en código, pendiente rotación)
- 4 archivos con XAVIER_TOKEN de producción hardcodeado (scripts/gestalt-xavier-cycle.sh, gestalt-vfs-env.sh, wave-orchestrator.py, gestalt_core/.../agent.rs) — **FIXED en commit fda39d7**
- **El token sigue en git history** → ROTAR el token de Xavier (acción manual de Belal)

### 🔴 Realidad de features (inflación 85.7% → 41%)
- features.json usa campos no canónicos (evidence) → el escáner no los lee
- Falta mapear a implemented_in/tests/status o documentar tests reales

### 🟡 Arquitectura BD (escala del bus)
- Dedup O(n) con parse JSON por insert → columna `dedup_hash` + índice
- Escrituras serializadas (Mutex + Exclusive) → OK para decenas/s, cuello con fan-out grande
- Sin retención/prune/VACUUM → job `gestalt bus prune`

### 🟡 Interconexión de agentes (el objetivo)
- gestalt-agent.sh existe en ~/scripts pero NO está versionado en el repo
- Falta: launcher real `gestalt agent exec`, suscripción con filtros, unificar canales WS/HTTP, registro de capacidades, encadenamiento

### 🟡 Vestigios
- gestalt-merge: 1 LOC (casi vacío)
- gestalt_swarm: excluido (legacy Python)
- gestalt-wasm: FASE3 pendiente, no compila (issues #466/#467)

## 3. ISSUES — Estado (12 → 7 abiertos)

Cerrados hoy (PRs merged verificados): #492→#500, #491→#503, #487→#495, #486→#497, #485→#496

Quedan abiertos:
- #471, #468 — features.json desync (audit) → este informe los atiende
- #467, #466 — gestalt-wasm FASE3
- #474, #473 — swal-agent-runner / software-factory (OTROS repos)
- #336-#341, #349-#352 — HARDEN legacy (merge flow, unsafe, deps)

## 4. PLAN DE AFINACIÓN PRIORIZADO (Kimi + auditoría)

| # | Prioridad | Tarea | Esfuerzo | Cierra |
|---|-----------|-------|----------|--------|
| 1 | **P0** | Rotar XAVIER_TOKEN + verificar 0 leaks en repo | 30min | Seguridad |
| 2 | **P0** | `gestalt agent exec` — launcher real (PRE Xavier → run → bus → POST) | 6h | Adopción del bus |
| 3 | **P0** | Alinear features.json a campos canónicos + documentar tests reales | 2h | #471/#468, % real |
| 4 | **P1** | Columna dedup_hash + índice compuesto (dedup O(1)) | 2-3h | Escala del bus |
| 5 | **P1** | `GET /api/events?agent=&type=&project=&after_seq=` + paginación cursor | 4h | Interconexión |
| 6 | **P1** | Unificar canales: un modelo de evento, 2 adaptadores (HTTP+WS) | 4h | #486 schema |
| 7 | **P1** | `gestalt bus prune` — retención (hot window 90d + archive Xavier) | 3h | Crecimiento |
| 8 | **P2** | Registry capabilities + POST /api/task routing declarativo | 8h | Orquestador |
| 9 | **P2** | Encadenamiento on_event (spec TOML declarativo) | 8h | Pipeline |
| 10 | **P2** | gestalt-wasm FASE3 (issues #466/#467) o decidir deprecar | 2-3d | FASE3 |
| 11 | **P2** | Cerrar HARDEN legacy #336-341, #349-352 | 4-8h | Deuda |

**Total P0-P1: ~21h · P2: ~25h + wasm**

## 5. MODELO DE NEGOCIO (veredicto Kimi)

Gestalt = tool pública del ecosistema. **Correcto no monetizarla**:
- Es la vitrina técnica verificable de SWAL (VFS, merge, trazabilidad)
- Cada run alimenta Xavier → memoria semántica = activo no copiable
- Monetizar la RED, no el tool: "Gestalt Pro = Gestalt + nodo SWAL activo" (memoria a escala, routing cross-mesh, auditoría de trazas) — cumple "Pro = nodo, nunca Stripe"
- Riesgo: si se vuelve infra crítica, el protocolo del bus debe ser spec abierta del ecosistema, no propiedad del repo

## 6. INTEGRACIÓN BD (veredicto Kimi)

El 3-tier DashMap → SQLite → Xavier es CORRECTO (µs/1ms/50-200ms):
- Bus persiste en SQLite (fuente de verdad con cursor seq) ✓ — no migrar a Xavier como destino primario
- NO migrar a Postgres — rompería el modelo local-first
- Faltantes: índice dedup_hash, retención, paginación cursor, backfill sweep (mencionado en event_bus.rs pero no implementado)

---
*Generado por Hermes + análisis Kimi CLI (session_5f780b70) — 2026-08-06*
