#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────
# gestalt-vfs-env.sh — Herramienta VFS para desarrollo aislado
#
# Crea entornos de desarrollo aislados por proyecto usando rsync
# para simular un overlay filesystem sobre un directorio base.
# Proporciona isolation (rsync copy-on-write), diff capture,
# y descarte limpio sin afectar el directorio original.
#
# Integra con Xavier para persistir metadata de sesiones y
# ejecuciones (kind=session / kind=execution).
#
# Uso:
#   gestalt-vfs-env.sh create   <proyecto>   Crear entorno aislado
#   gestalt-vfs-env.sh run      <comando>    Ejecutar dentro del VFS
#   gestalt-vfs-env.sh capture  [proyecto]   Capturar diff contra base
#   gestalt-vfs-env.sh destroy  [proyecto]   Descartar entorno
#   gestalt-vfs-env.sh status   [proyecto]   Mostrar estado
# ──────────────────────────────────────────────────────────────────
set -euo pipefail

# ═══ Config ═══════════════════════════════════════════════════════
GESTALT_DIR="${GESTALT_DIR:-$HOME/proyectosSWAL/gestalt}"
VFS_DIR="${VFS_DIR:-$HOME/.gestalt/vfs}"
VFS_BASE="$VFS_DIR/base"
VFS_OVERLAY="$VFS_DIR/overlay"
VFS_META="$VFS_DIR/meta"
TIMESTAMP="$(date +%s)"
TS_HUMAN="$(date -Iseconds)"

# Xavier config (misma convención que gestalt-xavier-cycle.sh)
XAVIER_URL="${XAVIER_URL:-http://127.0.0.1:8006}"
XAVIER_TOKEN="${XAVIER_TOKEN:-016d5454d90d1dc1711abc7ce008fc0d67622b569c3059e01b86f3228b3ce34a}"

# ═══ Colores ══════════════════════════════════════════════════════
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ═══ Help ═════════════════════════════════════════════════════════
show_help() {
    cat <<'HELP'
Usage: gestalt-vfs-env.sh <command> [options]

Commands:
  create   <project-path>   Create isolated VFS environment
  run      <command...>     Run command inside current VFS overlay
  capture  [project]        Show diff between overlay and base
  destroy  [project]        Remove overlay environment
  status   [project]        Show environment status
  --help                    Show this help

Examples:
  gestalt-vfs-env.sh create ~/projects/mi-proyecto
  gestalt-vfs-env.sh run "agy --edit main.rs fix typo"
  gestalt-vfs-env.sh capture
  gestalt-vfs-env.sh destroy

Environment:
  GESTALT_DIR     Gestalt project root (default: $HOME/proyectosSWAL/gestalt)
  VFS_DIR         VFS storage root (default: $HOME/.gestalt/vfs)
  XAVIER_URL      Xavier endpoint   (default: http://127.0.0.1:8006)
  XAVIER_TOKEN    Xavier auth token

Filesystem layout:
  ~/.gestalt/vfs/
    base/          ← Snapshots of original project (read-only reference)
    overlay/       ← Per-project overlay (writable, discarded on destroy)
    meta/          ← Metadata JSON files per environment

How it works:
  - 'create' copies the project to ~/.gestalt/vfs/base/ as a reference
    and creates an overlay copy for working
  - 'run' executes commands inside the overlay directory
  - 'capture' runs diff -r between base and overlay
  - 'destroy' removes the overlay and meta (base is kept as cache)
HELP
}

# ═══ Helpers ══════════════════════════════════════════════════════

# Derive a safe directory name from a project path
_safe_name() {
    echo "$1" | sed 's|^/||g; s|/|_|g; s|[^a-zA-Z0-9_-]|_|g'
}

# Get project name from first argument or current env
_project_name() {
    if [ -n "${VFS_CURRENT_PROJECT:-}" ]; then
        echo "$VFS_CURRENT_PROJECT"
    else
        echo ""
    fi
}

# Check if an environment exists
_env_exists() {
    local name="$1"
    [ -d "$VFS_OVERLAY/$name" ] && [ -f "$VFS_META/$name.json" ]
}

# Resolve project arg: use argument or current project from env
_resolve_project() {
    local name="${1:-}"
    if [ -z "$name" ]; then
        name="$(_project_name)"
    fi
    if [ -z "$name" ]; then
        echo "❌ No project specified. Use: $0 <command> <project-path>" >&2
        exit 1
    fi
    echo "$name"
}

# Xavier: PRE-search (buscar contexto existente)
xavier_pre_search() {
    local query="$1"
    if ! curl -sf -4 -X POST "$XAVIER_URL/v1/memories/search" \
        -H "Content-Type: application/json" \
        -H "X-Xavier-Token: $XAVIER_TOKEN" \
        -d "{\"query\":\"${query}\",\"limit\":3}" 2>/dev/null; then
        echo '{"results":[]}'
    fi
}

# Xavier: POST-store (archivar metadata)
xavier_store() {
    local content="$1"
    local path="$2"
    local kind="$3"
    local metadata="$4"
    curl -sf -4 -X POST "$XAVIER_URL/v1/memories" \
        -H "Content-Type: application/json" \
        -H "X-Xavier-Token: $XAVIER_TOKEN" \
        -d "{
            \"content\": $(echo "$content" | jq -Rs .),
            \"path\": $(echo "$path" | jq -Rs .),
            \"kind\": $(echo "$kind" | jq -Rs .),
            \"metadata\": $metadata
        }" 2>/dev/null || echo '{"status":"error","message":"Xavier unavailable"}'
}

# ═══ Command: create ═══════════════════════════════════════════════
cmd_create() {
    local project_path="${1:-}"
    if [ -z "$project_path" ]; then
        echo "❌ Usage: $0 create <project-path>"
        exit 1
    fi
    if [ ! -d "$project_path" ]; then
        echo "❌ Project directory does not exist: $project_path"
        exit 1
    fi

    # Resolve to absolute path
    project_path="$(cd "$project_path" && pwd)"
    local project_name
    project_name="$(_safe_name "$project_path")"
    local project_slug
    project_slug="$(basename "$project_path")"

    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║   🏗️  VFS: Creando entorno aislado              ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo ""
    echo "   Proyecto:  $project_slug"
    echo "   Ruta:      $project_path"
    echo "   ID:        $project_name"
    echo ""

    # Build directory structure
    mkdir -p "$VFS_BASE" "$VFS_OVERLAY" "$VFS_META"

    # ── Phase 1: Snapshot base (if not already cached) ──
    local base_path="$VFS_BASE/$project_name"
    local overlay_path="$VFS_OVERLAY/$project_name"
    local meta_path="$VFS_META/$project_name.json"

    if [ ! -d "$base_path" ]; then
        echo "📦 [1/3] Creando snapshot base (rsync)..."
        rsync -a --delete "$project_path/" "$base_path/" 2>/dev/null || {
            # Fallback: cp -a if rsync not available
            cp -a "$project_path" "$base_path" 2>/dev/null || {
                echo "❌ Error: rsync no disponible y cp falló" >&2
                exit 1
            }
        }
        echo "       ✅ Snapshot base creado en: $base_path"
    else
        echo "📦 [1/3] Snapshot base ya existe (usando caché)"
    fi

    # ── Phase 2: Create writable overlay ──
    echo "📝 [2/3] Creando overlay de trabajo..."
    if [ -d "$overlay_path" ]; then
        echo "       ⚠️  Overlay existente detectado. ¿Sobrescribir? [y/N]"
        read -r confirm
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo "       ❌ Cancelado. Use 'destroy' primero o especifique otro proyecto."
            exit 1
        fi
        rm -rf "$overlay_path"
    fi

    cp -a "$base_path" "$overlay_path"
    echo "       ✅ Overlay creado en: $overlay_path"

    # ── Phase 3: Save metadata & Xavier integration ──
    echo "💾 [3/3] Guardando metadata..."

    local meta_json
    meta_json="$(cat <<EOF
{
  "project_name": "$project_slug",
  "project_path": "$project_path",
  "vfs_id": "$project_name",
  "base_path": "$base_path",
  "overlay_path": "$overlay_path",
  "created_at": "$TS_HUMAN",
  "timestamp": $TIMESTAMP,
  "status": "active"
}
EOF
)"
    echo "$meta_json" > "$meta_path"
    echo "       ✅ Metadata guardada en: $meta_path"

    # Xavier PRE: buscar contexto relevante
    echo ""
    echo "🔍 Buscando contexto en Xavier..."
    local pre_result
    pre_result="$(xavier_pre_search "$project_slug")"
    local result_count
    result_count="$(echo "$pre_result" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('results',[])))" 2>/dev/null || echo "0")"
    echo "       📄 $result_count resultados relevantes en Xavier"

    # Xavier POST: registrar creación del entorno
    local post_result
    post_result="$(xavier_store \
        "VFS Environment created for project '$project_slug'
Path: $project_path
Overlay: $overlay_path
Status: active
Created: $TS_HUMAN" \
        "gestalt/vfs/env/$project_name" \
        "session" \
        "$meta_json")"
    local xavier_id
    xavier_id="$(echo "$post_result" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('id','?'))" 2>/dev/null || echo "?")"
    echo "       ✅ Archivado en Xavier (session): $xavier_id"

    # Export variable for subequent commands
    echo ""
    echo "   💡 Exporta esta variable para comandos subsecuentes:"
    echo "      export VFS_CURRENT_PROJECT=\"$project_name\""
    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║   ✅ Entorno VFS creado exitosamente            ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo "   Trabaja dentro de:"
    echo "     $overlay_path"
    echo "   Commands disponibles: run, capture, destroy"
    echo ""
}

# ═══ Command: run ═════════════════════════════════════════════════
cmd_run() {
    if [ $# -eq 0 ]; then
        echo "❌ Usage: $0 run <command...>"
        echo "   Ej:  $0 run agy --edit main.rs fix typo"
        echo "   Ej:  $0 run \"make build\""
        exit 1
    fi

    local project_name
    project_name="$(_project_name)"
    if [ -z "$project_name" ]; then
        echo "❌ No active VFS environment."
        echo "   Exporta VFS_CURRENT_PROJECT o usa 'create' primero."
        echo "   $0 status        — para ver entornos disponibles"
        exit 1
    fi

    local overlay_path="$VFS_OVERLAY/$project_name"
    local base_path="$VFS_BASE/$project_name"

    if [ ! -d "$overlay_path" ]; then
        echo "❌ Overlay no encontrado: $overlay_path"
        echo "   Usa 'create' primero o verifica VFS_CURRENT_PROJECT"
        exit 1
    fi

    local command="$*"
    local command_start
    command_start="$(date +%s)"

    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║   🚀 VFS: Ejecutando comando aislado            ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo ""
    echo "   Proyecto:  $(basename "$overlay_path")"
    echo "   Overlay:   $overlay_path"
    echo "   Comando:   $command"
    echo ""

    # ── Run command inside overlay ──
    echo "▸ Ejecutando..."
    echo "───────────────────────────────────────────────"
    set +e
    (
        cd "$overlay_path"
        eval "$command"
    )
    local exit_code=$?
    set -euo pipefail
    echo "───────────────────────────────────────────────"
    local command_end
    command_end="$(date +%s)"
    local duration=$((command_end - command_start))

    echo ""
    echo "   ⏱️  Duración: ${duration}s"
    echo "   🔚 Exit code: $exit_code"

    # ── Capture changes automatically after run ──
    if [ -d "$base_path" ]; then
        local changed_files
        changed_files="$(LC_ALL=C diff -rq "$base_path" "$overlay_path" 2>/dev/null | grep -v "^Only in .*\.git" | head -20 || true)"
        local changed_count
        changed_count="$(echo "$changed_files" | grep -c '^' 2>/dev/null || echo "0")"

        if [ "$changed_count" -gt 0 ]; then
            echo "   📝 Archivos modificados ($changed_count):"
            echo "$changed_files" | while IFS= read -r line; do
                echo "       $line"
            done | head -10

            # Archive in Xavier
            local diff_output
            diff_output="$(LC_ALL=C diff -ru "$base_path" "$overlay_path" 2>/dev/null | head -200 || true)"
            xavier_store \
                "VFS Run: $command
Duration: ${duration}s
Exit code: $exit_code
Project: $(basename "$overlay_path")
Changed files: $changed_count

Changes:
$diff_output" \
                "gestalt/vfs/run/${project_name}/${command_start}" \
                "execution" \
                "{\"project\":\"$(basename "$overlay_path" | jq -Rs .)\",\"command\":$(echo "$command" | jq -Rs .),\"duration\":$duration,\"exit_code\":$exit_code,\"changed_files\":$changed_count,\"timestamp\":$command_start}" \
                > /dev/null 2>&1 || true
        else
            echo "   ✅ Sin cambios detectados."
        fi
    fi

    echo ""
    if [ $exit_code -eq 0 ]; then
        echo "✅ Comando completado exitosamente."
    else
        echo "⚠️  Comando finalizó con código $exit_code"
    fi
    echo ""

    return $exit_code
}

# ═══ Command: capture ═════════════════════════════════════════════
cmd_capture() {
    local project_name
    project_name="$(_resolve_project "${1:-}")"
    local base_path="$VFS_BASE/$project_name"
    local overlay_path="$VFS_OVERLAY/$project_name"

    if [ ! -d "$overlay_path" ]; then
        echo "❌ Overlay no encontrado: $overlay_path"
        echo "   Entornos disponibles:"
        cmd_status
        exit 1
    fi
    if [ ! -d "$base_path" ]; then
        echo "⚠️  Base snapshot no encontrada. No se puede calcular diff."
        exit 1
    fi

    local diff_report
    diff_report="$(mktemp)"
    local diff_summary
    diff_summary="$(mktemp)"

    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║   📊 VFS: Capturando cambios                    ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo ""
    echo "   Proyecto:  $(basename "$overlay_path")"
    echo "   Base:      $base_path"
    echo "   Overlay:   $overlay_path"
    echo ""

    # ── Summary: changed files ──
    echo "📋 Archivos cambiados:"
    echo "───────────────────────────────────────────────"
    LC_ALL=C diff -rq "$base_path" "$overlay_path" 2>/dev/null \
        | grep -v "^Only in .*\.git" \
        | sort > "$diff_summary" || true

    local file_count
    file_count="$(wc -l < "$diff_summary" 2>/dev/null | tr -d ' ')"
    if [ "$file_count" -eq 0 ] || [ -z "$file_count" ]; then
        echo "   ✅ No hay cambios entre base y overlay."
        echo ""
        rm -f "$diff_report" "$diff_summary"
        return 0
    fi

    local added=0 modified=0 deleted=0
    while IFS= read -r line; do
        if echo "$line" | grep -q "^Files "; then
            modified=$((modified + 1))
            echo "   ✏️  Modificado: $(echo "$line" | sed 's/^Files //; s/ and .*//')"
        elif echo "$line" | grep -q "^Only in $overlay_path"; then
            added=$((added + 1))
            local f
            f="$(echo "$line" | sed "s|^Only in $overlay_path||; s|: ||")"
            echo "   ➕ Añadido:    $f"
        elif echo "$line" | grep -q "^Only in $base_path"; then
            deleted=$((deleted + 1))
            local f2
            f2="$(echo "$line" | sed "s|^Only in $base_path||; s|: ||")"
            echo "   ➖ Eliminado:  $f2"
        fi
    done < "$diff_summary"

    echo ""
    echo "   ──────────────────────────"
    echo "   Resumen: +$added añadidos, ✏️ $modified modificados, -$deleted eliminados"
    echo ""

    # ── Full diff ──
    echo "🔍 Diff completo (primeras 100 líneas):"
    echo "───────────────────────────────────────────────"
    diff -ru "$base_path" "$overlay_path" 2>/dev/null \
        | grep -v "^Only in .*\.git" \
        | head -100 > "$diff_report" || true

    if [ -s "$diff_report" ]; then
        cat "$diff_report"
        local diff_lines
        diff_lines="$(wc -l < "$diff_report" | tr -d ' ')"
        if [ "$diff_lines" -ge 100 ]; then
            echo "   ... (diff truncado a 100 líneas)"
        fi
    fi

    # ── Archive in Xavier ──
    local total=$((added + modified + deleted))
    if [ "$total" -gt 0 ]; then
        local diff_content
        diff_content="$(cat "$diff_report" | head -500)"
        xavier_store \
            "VFS Capture for project '$(basename "$overlay_path")'
Added: $added | Modified: $modified | Deleted: $deleted

$diff_content" \
            "gestalt/vfs/capture/${project_name}/${TIMESTAMP}" \
            "execution" \
            "{\"project\":$(basename "$overlay_path" | jq -Rs .),\"files_added\":$added,\"files_modified\":$modified,\"files_deleted\":$deleted,\"timestamp\":$TIMESTAMP}" \
            > /dev/null 2>&1 || true
        echo "   ✅ Cambios archivados en Xavier"
    fi

    echo ""
    echo "╚══════════════════════════════════════════════════╝"
    echo ""

    rm -f "$diff_report" "$diff_summary"
}

# ═══ Command: destroy ═════════════════════════════════════════════
cmd_destroy() {
    local project_name
    project_name="$(_resolve_project "${1:-}")"
    local overlay_path="$VFS_OVERLAY/$project_name"
    local meta_path="$VFS_META/$project_name.json"

    if [ ! -d "$overlay_path" ] && [ ! -f "$meta_path" ]; then
        echo "❌ No se encontró entorno para: $project_name"
        echo "   Entornos disponibles:"
        cmd_status
        exit 1
    fi

    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║   🗑️  VFS: Destruyendo entorno                  ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo ""
    echo "   Proyecto: $project_name"
    echo ""

    # Optional capture before destroy
    if [ -d "$overlay_path" ]; then
        echo "   ¿Capturar cambios antes de destruir? [y/N]"
        read -r capture_before
        if [ "$capture_before" = "y" ] || [ "$capture_before" = "Y" ]; then
            cmd_capture "$project_name"
        fi
    fi

    # Archive in Xavier
    local meta_content
    if [ -f "$meta_path" ]; then
        meta_content="$(cat "$meta_path")"
    else
        meta_content="{}"
    fi

    xavier_store \
        "VFS Environment destroyed for project '$project_name'
Destroyed at: $TS_HUMAN" \
        "gestalt/vfs/destroy/${project_name}/${TIMESTAMP}" \
        "execution" \
        "$meta_content" \
        > /dev/null 2>&1 || true

    # Remove overlay
    if [ -d "$overlay_path" ]; then
        rm -rf "$overlay_path"
        echo "   ✅ Overlay eliminado: $overlay_path"
    else
        echo "   ⚠️  Overlay no encontrado (ya limpiado)"
    fi

    # Remove meta
    if [ -f "$meta_path" ]; then
        rm -f "$meta_path"
        echo "   ✅ Metadata eliminada: $meta_path"
    fi

    echo ""
    echo "✅ Entorno destruido exitosamente."
    echo "   (Snapshot base se conserva en caché)"
    echo ""
}

# ═══ Command: status ══════════════════════════════════════════════
cmd_status() {
    local filter="${1:-}"

    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║   🔍 VFS: Estado de entornos                    ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo ""

    if [ ! -d "$VFS_META" ] || [ -z "$(ls -A "$VFS_META" 2>/dev/null)" ]; then
        echo "   📭 No hay entornos VFS activos."
        echo ""
        echo "   Crea uno con:"
        echo "     $0 create <project-path>"
        echo ""
        return 0
    fi

    local found=0
    for meta_file in "$VFS_META"/*.json; do
        [ -f "$meta_file" ] || continue
        local name
        name="$(basename "$meta_file" .json)"

        # Filter by name if requested
        if [ -n "$filter" ] && [ "$name" != "$filter" ]; then
            continue
        fi

        found=$((found + 1))
        local overlay_path="$VFS_OVERLAY/$name"
        local base_path="$VFS_BASE/$name"

        # Read metadata
        local project_name project_path created_at
        project_name="$(python3 -c "import json; print(json.load(open('$meta_file')).get('project_name','?'))" 2>/dev/null || echo "?")"
        project_path="$(python3 -c "import json; print(json.load(open('$meta_file')).get('project_path','?'))" 2>/dev/null || echo "?")"
        created_at="$(python3 -c "import json; print(json.load(open('$meta_file')).get('created_at','?'))" 2>/dev/null || echo "?")"

        # Check overlay health
        local status_icon overlay_status="⚠️  No encontrado"
        if [ -d "$overlay_path" ]; then
            local overlay_size
            overlay_size="$(du -sh "$overlay_path" 2>/dev/null | cut -f1 || echo "?")"
            overlay_status="✅ Activo ($overlay_size)"
            status_icon="🟢"
        else
            status_icon="🔴"
        fi

        # Calculate diff stats
        local diff_stats="N/A"
        if [ -d "$base_path" ] && [ -d "$overlay_path" ]; then
            local changed
            changed="$(diff -rq "$base_path" "$overlay_path" 2>/dev/null | grep -v "^Only in .*\.git" | wc -l | tr -d ' ' || true)"
            if [ "${changed:-0}" -gt 0 ] 2>/dev/null; then
                diff_stats="$changed archivos cambiados"
            else
                diff_stats="sin cambios"
            fi
        fi

        echo "   📁 [$name]"
        echo "      Proyecto:   $project_name"
        echo "      Ruta:       $project_path"
        echo "      Creado:     $created_at"
        echo "      Overlay:    $overlay_status"
        echo "      Cambios:    $diff_stats"
        echo ""
    done

    if [ "$found" -eq 0 ]; then
        if [ -n "$filter" ]; then
            echo "   🔍 No se encontró entorno: $filter"
        else
            echo "   📭 No hay entornos VFS activos."
        fi
        echo ""
    fi

    # Show current env if set
    if [ -n "${VFS_CURRENT_PROJECT:-}" ]; then
        echo "   🎯 Entorno activo (VFS_CURRENT_PROJECT): $VFS_CURRENT_PROJECT"
        echo ""
    fi
}

# ═══ Main ═════════════════════════════════════════════════════════
main() {
    mkdir -p "$VFS_DIR"

    local cmd="${1:-help}"
    shift 2>/dev/null || true

    case "$cmd" in
        create)
            cmd_create "$@"
            ;;
        run)
            cmd_run "$@"
            ;;
        capture)
            cmd_capture "$@"
            ;;
        destroy)
            cmd_destroy "$@"
            ;;
        status)
            cmd_status "$@"
            ;;
        --help|-h|help)
            show_help
            ;;
        *)
            echo "❌ Comando desconocido: $cmd"
            echo "   Usa '$0 --help' para ver comandos disponibles."
            exit 1
            ;;
    esac
}

main "$@"
