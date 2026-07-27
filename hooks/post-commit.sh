#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt Post-Commit Hook — Index commit in Xavier
# ──────────────────────────────────────────────────────────
set -euo pipefail

COMMIT_MSG=$(git log -1 --pretty=%B)
COMMIT_HASH=$(git rev-parse HEAD)
BRANCH=$(git rev-parse --abbrev-ref HEAD)

gestalt xavier add "Commit: $COMMIT_HASH on $BRANCH
$COMMIT_MSG" --path "git/commits/$COMMIT_HASH" 2>/dev/null || true
