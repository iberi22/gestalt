# 📋 Feedback de Kimi K3 — Revisión del Sistema Gestalt

> Generado por Kimi CLI (K3) el 2026-07-26
> Revisión completa de arquitectura, registry, tiny agents, rate limits y prioridades

---

## 🔴 Hallazgos Críticos

### 1. Ciclo `gestalt xavier cycle` viola AGENTS.md
**Problema:** Ejecuta `sh -c "$agent_cmd"` sin VFS isolation — viola la regla #1 de AGENTS.md (agentes deben pasar por VFS).
**Solución:** El cycle debe crear VFS overlay, ejecutar dentro, y capturar diffs via AgentWrapper.

### 2. Cycle archiva solo el COUNT, no el contenido
**Problema:** `format!("Cycle: {}\nResults: {}", task, results)` donde `results` es un número. El stdout del agente NO se archiva.
**Solución:** Archivar stdout completo + BlockEdits generados.

### 3. TPM tracking es muerto
**Problema:** `record_usage` guarda `current_tpm` pero NADIE lo lee. Solo se filtra por RPM.
**Solución:** Añadir filtro TPM en `select_agent`.

### 4. Provider rate limits inertes
**Problema:** `providers` en TOML declarados pero nunca trackeados en Rust. Solo per-agent.
**Solución:** Trackear a nivel provider también.

### 5. Tiny agents sobreingeniería
**Problema:** Usar LLM (0.5B-3.8B) para `delete-line` es excesivo — operación determinística.
**Propuesta:** Tiny model solo decide POSICIÓN; la aplicación es mecánica.

### 6. Dataset de fine-tuning NO existe
**Problema:** El plan depende de BlockEdits de AgentWrapper, que no produce datos aún.
**Solución:** Implementar captura de datos primero, fine-tuning después.

## 🟡 Hallazgos Medios

### 7. 11 agentes, no 12
Discrepancia entre el claim y el TOML real.

### 8. `task.len() < 200` bug
Selecciona ANY tiny agent, no el que tenga la capacidad correcta.

### 9. Sin fallback para Ocupado
Si un agente se marca Ocupado y crashea, nunca se recupera. Sin timeout.

### 10. Rate limits sin sliding window
Reset fijo cada 60s permite bursts en bordes de ventana.

### 11. TPM suma > provider cap
agent-cli-low 4M TPM + otros = 5.7M > 2M del provider OpenRouter.

### 12. SKILL.md vs TOML inconsistentes
Hermes: SKILL dice 200 RPM, TOML dice 60 RPM.

## 🟢 Recomendaciones de Prioridades

Kimi K3 propone REORDENAR las fases:

| Orden Actual (fase-de-afinacion.md) | Orden Propuesto por Kimi | Razón |
|-------------------------------------|--------------------------|-------|
| 1. Xavier Embedding | **3. gestalt-merge fix** | Sin merge flow, la entrega sin conflictos es vapor |
| 2. Tiny Agents FT | **1. Cycle VFS fix** | La violación #1 bloquea el uso real |
| 3. gestalt-merge fix | **2. Cycle archivar contenido** | Sin datos archivados, el ciclo no sirve |
| 4. Rate Limits | **4. Xavier Embedding** | Barato, desbloquea búsqueda |
| 5. XavierClient Kinds | **5. TPM tracking fix** | Dead code removal |
| 6. MCP Backend | **6. Provider rate limits** | Completar el modelo |
| 7. Tests | **Interleaved** | Tests deben acompañar cada fix |
| 8. CLI Autocomplete | **8. Rate limits sliding window** | Mejora operativa |
| 9. Dashboard | **9. Tiny Agents FT** | Alto riesgo, baja certeza — último |

---

## ✅ Lo que Kimi APROBÓ del sistema

1. **Agent Registry en Rust** — Sólido, tipado, con tests
2. **4 routing strategies** — Correctas, bien implementadas
3. **Separación Hermes/Gestalt** — Conceptualmente acertada
4. **VFS implementado** — OverlayFs + StateDbVfs + MemoryFs reales
5. **Xavier PRE/POST** — Implementado en router.rs
6. **LiveConflictDetector** — Bien diseñado con WsEvent
7. **Git hooks** — post-commit + pre-push correctos
8. **12 agentes con roles** — Bien diferenciados (salvo tiny-search)
