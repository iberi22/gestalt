#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt ↔ Xavier Cycle — Production Wrapper
# Uso: ./gestalt-xavier.sh "tu consulta o tarea"
# ──────────────────────────────────────────────────────────
set -euo pipefail

# ═══ Config ═══════════════════════════════════════════════
XAVIER_URL="${XAVIER_URL:-http://127.0.0.1:8006}"
XAVIER_TOKEN="${XAVIER_TOKEN:-}"
GESTALT_DIR="$HOME/proyectosSWAL/gestalt"
QUERY="${1:-}"
TIMESTAMP="$(date +%s)"

if [ -z "$QUERY" ]; then
    echo "❌ Uso: $0 <query o tarea>"
    echo "   Ej:  $0 \"indexar documentos de arquitectura en Xavier\""
    exit 1
fi

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   🔍 Gestalt ↔ Xavier Cycle                     ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# ═══ Fase 1: PRE — Buscar contexto en Xavier ═══════════
echo "📖 [1/4] PRE — Consultando Xavier..."
echo "     Query: $QUERY"
echo ""

PRE_RESULT=$(curl -s -4 -X POST "$XAVIER_URL/v1/memories/search" \
  -H "Content-Type: application/json" \
  -H "X-Xavier-Token: $XAVIER_TOKEN" \
  -d "{\"query\":\"$QUERY\",\"limit\":5}" 2>/dev/null)

echo "$PRE_RESULT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
results = d.get('results', [])
print(f'     📄 {len(results)} resultados de Xavier')
for i, r in enumerate(results[:3], 1):
    meta = r.get('metadata', {})
    kind = meta.get('kind', '?')
    memory = r.get('memory', '')
    title = memory.split(chr(10))[0][:80] if memory else '?'
    print(f'        {i}. [{kind}] {title}')
" 2>/dev/null || echo "     ⚠️ Sin resultados"

# ═══ Fase 2: Construir contexto aumentado ═══════════════
echo ""
echo "📝 [2/4] Construyendo contexto para subagente..."

# Si Xavier tiene contexto, lo usamos; si no, usamos contexto directo
if echo "$PRE_RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); exit(0 if d.get('results') else 1)" 2>/dev/null; then
    CONTEXT_SOURCE="Xavier (memoria persistente)"
else
    CONTEXT_SOURCE="consulta directa"
fi
echo "     Fuente: $CONTEXT_SOURCE"

# ═══ Fase 3: Ejecutar subagente ═════════════════════════
echo ""
echo "🤖 [3/4] Subagente listo para tarea..."
echo "     Tarea: $QUERY"
echo ""
echo "     Para lanzar el subagente desde Hermes:"
echo "     ─────────────────────────────────────"
echo "      delegate_task("
echo "        goal=\"$QUERY\""
echo "        context=\"Contexto de Xavier: \$XAVIER_CONTEXT\""
echo "      )"
echo ""

# ═══ Fase 4: POST — Archivar en Xavier ══════════════════
echo "💾 [4/4] POST — Archivando registro en Xavier..."

ARCHIVE_BODY="$(cat <<EOF
{
  "content": "Tarea ejecutada: $QUERY\nTimestamp: $(date -Iseconds)\nContexto: $CONTEXT_SOURCE",
  "path": "gestalt/cycle/$TIMESTAMP",
  "kind": "execution",
  "metadata": {
    "source": "gestalt-cli-cycle",
    "query": "$QUERY",
    "timestamp": "$(date -Iseconds)"
  }
}
EOF
)"

POST_RESULT=$(curl -s -4 -X POST "$XAVIER_URL/v1/memories" \
  -H "Content-Type: application/json" \
  -H "X-Xavier-Token: $XAVIER_TOKEN" \
  -d "$ARCHIVE_BODY" 2>/dev/null)

echo "$POST_RESULT" | python3 -c "
import sys,json
d = json.load(sys.stdin)
if d.get('status') == 'error':
    print(f'     ❌ Error: {d.get(\"message\",\"desconocido\")}')
else:
    mid = d.get('id', 'ok')
    print(f'     ✅ Archivado como: {mid}')
" 2>/dev/null || echo "     ⚠️ No se pudo archivar"

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   ✅ Ciclo Gestalt ↔ Xavier completado           ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""
echo "   Query: $QUERY"
echo "   Hora:  $(date)"
echo ""
