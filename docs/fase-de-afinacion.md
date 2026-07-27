# 🎯 Fase de Afinación — Gestalt Ecosystem

## Resumen de trabajo pendiente tras 4 oleadas

```
═══════════════════════════════════════════════════════════════════════════════
 🎯 FASE DE AFINACIÓN — Consolidado de tareas pendientes
═══════════════════════════════════════════════════════════════════════════════
```

---

## 🔴 Bloque 1 — Tiny Agents (Fine-Tuning de Modelos Locales)

**Estado:** 4 tiny agents definidos en `agent-registry.toml` pero SIN fine-tuning. Usan modelos base.

| Tiny Agent | Modelo Base | Tarea | Dataset Necesario |
|-----------|-------------|-------|-------------------|
| `tiny-insert-line` | phi-3-mini (~3.8B) | Insertar 1 línea exacta en posición | Ediciones reales de agy/opencode |
| `tiny-delete-line` | tinyllama-1.1b | Eliminar 1 línea exacta | Eliminaciones reales |
| `tiny-replace-line` | qwen2.5-coder-0.5b | Reemplazar 1 línea exacta | Reemplazos reales |
| `tiny-search` | all-MiniLM-L6-v2 | Búsqueda semántica | Embeddings de código |

**Pasos:**
1. Extraer BlockEdits reales desde AgentWrapper (ya captura diffs como Insert/Delete/Replace)
2. Crear dataset de entrenamiento: `[input: "path:line:context", output: "block_edit"]`
3. Fine-tune con LoRA (QLoRA para 0.5B-3.8B)
4. Validar con tests de regresión
5. Integrar en gestalt xavier cycle como agentes por defecto para ediciones

---

## 🔴 Bloque 2 — Xavier Embedding Fix

**Estado:** Xavier corriendo pero en modo `local-degraded` — embeddings no funcionales.

```json
{"status":"degraded","mode":"local-degraded","database":{"status":"ok"}}
```

**Problema:** Sin embeddings, las búsquedas semánticas no funcionan. Solo búsqueda por keywords.

**Pasos:**
1. Configurar proveedor de embeddings: `XAVIER_EMBEDDING_PROVIDER_MODE=cloud`
2. O verificar modelo local en Ollama
3. Re-indexar documentos existentes
4. Verificar que search devuelva resultados por relevancia

---

## 🔴 Bloque 3 — gestalt-merge + synapse-agentic

**Estado:** Ambos crates excluidos del workspace por errores de compilación pre-existentes.

**Pasos:**
1. `cargo check -p gestalt-merge` → diagnosticar errores
2. `cargo check -p synapse-agentic` → diagnosticar errores
3. Fix secuencial (sin romper dependencias)
4. Re-incluir en workspace members de Cargo.toml raíz
5. Integrar con gestalt-router via merge flow

---

## 🟡 Bloque 4 — Extender XavierClient Memory Kinds

**Estado:** `XavierClient` solo soporta `kind="run_result"` y `kind="session"`. Faltan `plan`, `execution`, `config`.

**Pasos:**
1. Añadir métodos a `XavierClient`:
   - `save_plan(content, path) -> kind="plan"`
   - `save_execution(content, path) -> kind="execution"`
   - `save_config(content, path) -> kind="config"`
2. Actualizar `gestalt xavier add` para aceptar --kind
3. Registrar estos kinds en Xavier (scoring/boosting)

---

## 🟡 Bloque 5 — Gestalt como Backend MCP

**Estado:** Hermes ↔ Gestalt sin protocolo formal (solo CLI).

**Pasos:**
1. Gestalt expone endpoint HTTP/MCP (basado en MCP adapter existente)
2. Hermes consume Gestalt como herramienta MCP
3. `OrchestrationRequest` → `OrchestrationResult` como contrato formal
4. Documentación en docs/hermes-gestalt-protocol.md (ya creado)

---

## 🟡 Bloque 6 — Rate Limits Persistentes

**Estado:** Rate limits en memoria volátil. Se pierden al reiniciar Gestalt.

**Pasos:**
1. Guardar `current_rpm`/`current_tpm` en SQLite (StateDb)
2. Cargar al iniciar Gestalt
3. Reset automático cada 60s
4. Dashboard `gestalt xavier stats` con histórico

---

## 🟢 Bloque 7 — Tests Faltantes

**Estado:** Sin tests para AgentWrapper, Registry edge cases, CLI integration.

| Módulo | Tests Existentes | Tests Faltantes |
|--------|-----------------|-----------------|
| AgentWrapper | 0 | parse_unified_diffs, apply_edit_to_vfs, BlockEdit edge cases |
| Registry | 3 básicos | Rate limit overflow, provider failover, routing edge cases |
| gestalt xavier CLI | 0 | Integration tests con Xavier real |
| Git hooks | 0 | post-commit, pre-push scenarios |

---

## 🟢 Bloque 8 — CLI Autocomplete + UX

**Estado:** Sin bash completion, sin help detallado.

**Pasos:**
1. `gestalt completion bash` (clap soporta autocomplete nativo)
2. Mejorar mensajes de error del CLI
3. `gestalt xavier --help` con ejemplos

---

## 🟢 Bloque 9 — Dashboard Web (Opcional)

**Estado:** No existe UI.

**Pasos:**
1. Panel simple con estado de agentes
2. Timeline de ejecuciones desde Xavier
3. Rate limits actuales

---

## 📋 Orden de Ejecución Propuesto

```
Fase 1: 🔴 Xavier Embedding Fix (dependencia crítica para todo lo demás)
Fase 2: 🔴 Tiny Agents Dataset + Fine-tuning (modelos locales)
Fase 3: 🔴 gestalt-merge + synapse-agentic fix (workspace completo)
Fase 4: 🟡 Rate Limits Persistentes (mejora operativa)
Fase 5: 🟡 XavierClient Memory Kinds (API completa)
Fase 6: 🟡 Gestalt MCP Backend (protocolo formal)
Fase 7: 🟢 Tests Faltantes (calidad)
Fase 8: 🟢 CLI Autocomplete (UX)
Fase 9: 🟢 Dashboard Web (visibilidad)
```

---

*Documento creado para revisión por Kimi K3*
