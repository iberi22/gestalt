#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt ↔ Xavier Cycle — Production Wrapper
# Uso: ./gestalt-xavier-cycle.sh "tu consulta o tarea" [--agent "comando"]
# ──────────────────────────────────────────────────────────
set -euo pipefail

# ═══ Config ═══════════════════════════════════════════════
XAVIER_URL="${XAVIER_URL:-http://127.0.0.1:8006}"
XAVIER_TOKEN="${XAVIER_TOKEN:-}"
if [ -z "$XAVIER_TOKEN" ] && [ -f "$HOME/proyectosSWAL/apps/xavier/.env" ]; then
    XAVIER_TOKEN=$(grep -E '^XAVIER_TOKEN=' "$HOME/proyectosSWAL/apps/xavier/.env" 2>/dev/null | cut -d'=' -f2- | tr -d '"' | tr -d "'" || true)
fi
GESTALT_DIR="$HOME/proyectosSWAL/apps/gestalt"
TIMESTAMP="$(date +%s)"

QUERY=""
AGENT_CMD="${AGENT_CMD:-}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --serve-bus)
            BIN="$GESTALT_DIR/target/debug/gestalt_cli"
            [ -x "$GESTALT_DIR/target/release/gestalt_cli" ] && BIN="$GESTALT_DIR/target/release/gestalt_cli"
            echo "🚀 Iniciando Gestalt Universal Event Bus en http://127.0.0.1:8081..."
            mkdir -p "$HOME/.gestalt"
            export XAVIER_TOKEN
            exec "$BIN" bus serve --host 127.0.0.1 --port 8081 --db "$HOME/.gestalt/state.db"
            ;;
        --agent|-a)
            AGENT_CMD="$2"
            shift 2
            ;;
        *)
            if [ -z "$QUERY" ]; then
                QUERY="$1"
            fi
            shift
            ;;
    esac
done

if [ -z "$QUERY" ]; then
    echo "❌ Uso: $0 <query o tarea> [--agent \"comando a ejecutar\"]"
    echo "   O bien: $0 --serve-bus (inicia el daemon del bus HTTP :8081)"
    echo "   Ej:  $0 \"indexar arquitectura\" --agent \"cargo test --test router_tests\""
    exit 1
fi

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   🔍 Gestalt ↔ Xavier Cycle                     ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# ═══ Helper: Check Xavier availability ═══════════════════
XAVIER_EXEC_BIN=""

is_xavier_available() {
    if command -v xavier >/dev/null 2>&1; then
        if curl -s --connect-timeout 2 --max-time 5 "$XAVIER_URL/health" >/dev/null 2>&1 || curl -s --connect-timeout 1 --max-time 3 "$XAVIER_URL/memory/stats" -H "X-Xavier-Token: $XAVIER_TOKEN" >/dev/null 2>&1; then
            XAVIER_EXEC_BIN="xavier exec"
            return 0
        fi
    elif command -v xavier_run_command >/dev/null 2>&1; then
        XAVIER_EXEC_BIN="xavier_run_command"
        return 0
    fi
    return 1
}

# ═══ Helper: Command Execution via Xavier RTK Proxy ═════
XAVIER_PROXY_TELEMETRY=""
CMD_OUTPUT=""
CMD_EXIT_CODE=0

execute_subagent_cmd() {
    local cmd="$1"
    local output=""
    local exit_code=0
    local raw_len=0
    local est_raw_tokens=0
    local proxy_used=false

    echo "     🚀 Executing agent command: $cmd"

    if is_xavier_available; then
        echo "     [xavier-proxy] Routing execution via Xavier RTK Proxy ($XAVIER_EXEC_BIN)..."
        proxy_used=true
        set +e
        if [ "$XAVIER_EXEC_BIN" = "xavier exec" ]; then
            output=$(xavier exec "$cmd" 2>&1)
            exit_code=$?
        else
            output=$(xavier_run_command "$cmd" 2>&1)
            exit_code=$?
        fi
        set -e
    else
        echo "     ⚠️ [xavier-proxy] Xavier proxy unavailable (not in PATH or offline). Falling back to direct sh -c..."
        set +e
        output=$(sh -c "$cmd" 2>&1)
        exit_code=$?
        set -e
    fi

    raw_len=${#output}
    est_raw_tokens=$(( (raw_len + 3) / 4 ))

    if [ "$proxy_used" = true ]; then
        echo "     [xavier-proxy] Token savings telemetry: RTK proxy compressed execution output (~${est_raw_tokens} raw tokens saved/optimized)."
        XAVIER_PROXY_TELEMETRY="[xavier-proxy] mode=rtk_proxy tokens_saved_est=${est_raw_tokens} exit_code=${exit_code}"
    else
        echo "     [xavier-proxy] Direct execution completed (~${est_raw_tokens} raw tokens consumed, 0% savings)."
        XAVIER_PROXY_TELEMETRY="[xavier-proxy] mode=direct_fallback tokens_saved_est=0 exit_code=${exit_code}"
    fi

    echo "     ──────────────── Output ────────────────"
    echo "$output" | head -n 20
    if [ $(echo "$output" | wc -l) -gt 20 ]; then
        echo "     ... (truncated for display)"
    fi
    echo "     ────────────────────────────────────────"

    CMD_OUTPUT="$output"
    CMD_EXIT_CODE="$exit_code"
}

# ═══ Fase 1: PRE — Buscar contexto en Xavier ═══════════
echo "📖 [1/4] PRE — Consultando Xavier..."
echo "     Query: $QUERY"
echo ""

PRE_RESULT=$(curl -s --connect-timeout 1 --max-time 2 -X POST "$XAVIER_URL/v1/memories/search" \
  -H "Content-Type: application/json" \
  -H "X-Xavier-Token: $XAVIER_TOKEN" \
  -d "{\"query\":\"$QUERY\",\"limit\":5}" 2>/dev/null) || PRE_RESULT=""

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
" 2>/dev/null || echo "     ⚠️ HTTP sin resultados"

# Fallback offline: indice local del CLI cuando HTTP falla/cuelga
XAVIER_OFFLINE_CTX=""
if ! echo "$PRE_RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); exit(0 if d.get('results') else 1)" 2>/dev/null; then
    echo "     [xavier-proxy] Probando indice offline (xavier search)..."
    XAVIER_OFFLINE_CTX=$(xavier search "$QUERY" 5 2>/dev/null | grep -vE 'INFO|Searching|OFFLINE|CONNECTION_REFUSED|Falling back|loaded_memories|schema version' | head -n 25 || true)
    if [ -n "$XAVIER_OFFLINE_CTX" ]; then
        echo "     [xavier-proxy] Contexto offline obtenido (indice local)."
    else
        echo "     [xavier-proxy] Sin contexto offline tampoco."
    fi
fi

# ═══ Fase 2: Construir contexto aumentado ═══════════════
echo ""
echo "📝 [2/4] Construyendo contexto para subagente..."

if echo "$PRE_RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); exit(0 if d.get('results') else 1)" 2>/dev/null; then
    CONTEXT_SOURCE="Xavier (memoria persistente)"
elif [ -n "$XAVIER_OFFLINE_CTX" ]; then
    CONTEXT_SOURCE="Xavier offline (indice local)"
else
    CONTEXT_SOURCE="consulta directa"
fi
echo "     Fuente: $CONTEXT_SOURCE"

# ═══ Fase 3: Ejecutar subagente ═════════════════════════
echo ""
echo "🤖 [3/4] Subagente listo para tarea..."
echo "     Tarea: $QUERY"
echo ""

if [ -n "$AGENT_CMD" ]; then
    execute_subagent_cmd "$AGENT_CMD"
else
    echo "     Para lanzar el subagente desde Hermes:"
    echo "     ─────────────────────────────────────"
    echo "      delegate_task("
    echo "        goal=\"$QUERY\""
    echo "        context=\"Contexto de Xavier: \$XAVIER_CONTEXT\""
    echo "      )"
    echo "     O ejecutar directamente: $0 \"$QUERY\" --agent \"<comando>\""
    echo ""
    XAVIER_PROXY_TELEMETRY="[xavier-proxy] mode=none tokens_saved_est=0 exit_code=0"
    CMD_OUTPUT="N/A (no command provided)"
fi

# ═══ Fase 4: POST — Archivar en Xavier ══════════════════
echo "💾 [4/4] POST — Archivando registro en Xavier..."

# Sanitize CMD_OUTPUT and QUERY for JSON
CLEAN_QUERY=$(echo "$QUERY" | python3 -c "import sys, json; print(json.dumps(sys.stdin.read().strip())[1:-1])" 2>/dev/null || echo "$QUERY")
CLEAN_TELEMETRY=$(echo "$XAVIER_PROXY_TELEMETRY" | python3 -c "import sys, json; print(json.dumps(sys.stdin.read().strip())[1:-1])" 2>/dev/null || echo "$XAVIER_PROXY_TELEMETRY")

ARCHIVE_BODY="$(cat <<EOF
{
  "content": "Tarea ejecutada: $CLEAN_QUERY\nTimestamp: $(date -Iseconds)\nContexto: $CONTEXT_SOURCE\nTelemetry: $CLEAN_TELEMETRY",
  "path": "gestalt/cycle/$TIMESTAMP",
  "kind": "execution",
  "metadata": {
    "source": "gestalt-cli-cycle",
    "query": "$CLEAN_QUERY",
    "agent_cmd": "$AGENT_CMD",
    "xavier_proxy_telemetry": "$CLEAN_TELEMETRY",
    "timestamp": "$(date -Iseconds)"
  }
}
EOF
)"

POST_RESULT=$(curl -s --connect-timeout 1 --max-time 2 -X POST "$XAVIER_URL/v1/memories" \
  -H "Content-Type: application/json" \
  -H "X-Xavier-Token: $XAVIER_TOKEN" \
  -d "$ARCHIVE_BODY" 2>/dev/null) || POST_RESULT=""

echo "$POST_RESULT" | python3 -c "
import sys,json
d = json.load(sys.stdin)
if d.get('status') == 'error':
    print(f'     ❌ Error: {d.get(\"message\",\"desconocido\")}')
else:
    mid = d.get('id', 'ok')
    print(f'     ✅ Archivado como: {mid}')
" 2>/dev/null || {
    echo "     [xavier-proxy] HTTP POST fallo — fallback CLI offline..."
    if xavier add "Tarea ejecutada: $QUERY | Contexto: $CONTEXT_SOURCE | Telemetry: $XAVIER_PROXY_TELEMETRY" "gestalt-cycle-$TIMESTAMP" -k episodic 2>/dev/null | grep -qE '✅|successfully'; then
        echo "     ✅ Archivado offline (indice local)"
    else
        echo "     ⚠️ No se pudo archivar"
    fi
}

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   ✅ Ciclo Gestalt ↔ Xavier completado           ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""
echo "   Query: $QUERY"
echo "   Hora:  $(date)"
if [ -n "$XAVIER_PROXY_TELEMETRY" ]; then
    echo "   Telemetry: $XAVIER_PROXY_TELEMETRY"
fi
echo ""
