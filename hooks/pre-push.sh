#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt Pre-Push Hook — Optional Gestalt validation
# ──────────────────────────────────────────────────────────
set -euo pipefail

if command -v gestalt &> /dev/null; then
  echo "🔍 Pre-push validation via Gestalt..."
  # Check if there are merge conflicts
  if git diff --check --cached | grep -q conflict; then
    echo "❌ Conflict detected! Run Gestalt conflict resolver first."
    exit 1
  fi
fi
exit 0
