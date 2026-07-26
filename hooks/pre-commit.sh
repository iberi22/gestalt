#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt Pre-Commit Hook — Rust Workspace Validation
# Versión: SUPER-ROBUSTO
# Reemplaza el hook pwsh que fallaba en NixOS
# ──────────────────────────────────────────────────────────
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$REPO_ROOT"

START_TIME=$(date +%s%N)
EXIT_CODE=0

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m'

# ── Utils ─────────────────────────────────────────────────
pass()   { echo -e "  ${GREEN}✅${NC} $1"; }
fail()   { echo -e "  ${RED}❌${NC} $1"; EXIT_CODE=1; }
warn()   { echo -e "  ${YELLOW}⚠️${NC} $1"; }
header() { echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

# Build fix for OpenSSL in Nix
BUILD_ENV="unset OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR"
BUILD_ENV+=" && PKG_CONFIG_PATH=\"\$(nix eval nixpkgs#openssl.dev --raw)/lib/pkgconfig\""

# ── Stage 1: Format Check ────────────────────────────────
header "📐 [1/5] Rust Format Check"
if eval "$BUILD_ENV && cargo fmt --all --check 2>&1"; then
    pass "cargo fmt: PASS"
else
    fail "cargo fmt: FAIL — run 'cargo fmt --all' to fix"
fi

# ── Stage 2: Clippy Lint Check ───────────────────────────
header "🔎 [2/5] Clippy Lint Check"
if eval "$BUILD_ENV && cargo clippy -p gestalt-state -p gestalt-ws --all-targets 2>&1" && eval "$BUILD_ENV && cargo clippy -p gestalt-router --lib 2>&1"; then
    pass "clippy: PASS"
else
    fail "clippy: FAIL — fix warnings before commit"
fi

# ── Stage 3: Build Check ─────────────────────────────────
header "📦 [3/5] Cargo Build Check"
if eval "$BUILD_ENV && cargo check --workspace 2>&1"; then
    pass "build: PASS"
else
    fail "build: FAIL — fix compilation errors"
fi

# ── Stage 4: Tests ───────────────────────────────────────
header "🧪 [4/5] Cargo Test (gestalt-state)"
if eval "$BUILD_ENV && cargo test -p gestalt-state -- --skip test_try_lock_exclusive 2>&1"; then
    pass "gestalt-state tests: PASS"
else
    fail "gestalt-state tests: FAIL"
fi

header "🧪 [4b/5] Cargo Test (gestalt-router WS)"
if eval "$BUILD_ENV && cargo test -p gestalt-router --test ws_tests -- --test-threads=1 2>&1"; then
    pass "WS tests: PASS"
else
    fail "WS tests: FAIL"
fi

header "🧪 [4c/5] Cargo Test (gestalt_cli)"
if eval "$BUILD_ENV && cargo test -p gestalt_cli agent_wrapper 2>&1"; then
    pass "agent_wrapper tests: PASS"
else
    fail "agent_wrapper tests: FAIL"
fi

header "🧪 [4d/5] Cargo Test (gestalt-ws)"
if eval "$BUILD_ENV && cargo test -p gestalt-ws 2>&1"; then
    pass "gestalt-ws tests: PASS"
else
    fail "gestalt-ws tests: FAIL"
fi

# ── Stage 5: Security & Sanity ────────────────────────────
header "🔒 [5/5] Security Checks"

ALL_GOOD=true

# 5a. No .env files
STAGED=$(git diff --cached --name-only)
for f in $STAGED; do
    if echo "$f" | grep -q '\.env$' && [ "$f" != ".env.example" ]; then
        fail "SECURITY: .env files should not be committed ($f)"
        ALL_GOOD=false
    fi
done

# 5b. No secrets in filenames
for f in $STAGED; do
    if echo "$f" | grep -qiE '(api.?key|token|secret|password|credential)'; then
        warn "$f may contain credentials"
    fi
done

# 5c. No large files (>1MB)
for f in $STAGED; do
    if [ -f "$f" ]; then
        SIZE=$(stat -c%s "$f" 2>/dev/null || stat -f%z "$f" 2>/dev/null)
        if [ "${SIZE:-0}" -gt 1048576 ]; then
            warn "Large file staged: $f ($((SIZE/1048576)) MB)"
        fi
    fi
done

if $ALL_GOOD; then
    pass "security: PASS"
fi

# ── Summary ──────────────────────────────────────────────
ELAPSED=$(( ($(date +%s%N) - START_TIME) / 1000000 ))
echo -e "\n${CYAN}═══════════════════════════════════════════${NC}"
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}  ✅ ALL CHECKS PASSED ($((ELAPSED/1000)).${ELAPSED: -3}s)${NC}"
else
    echo -e "${RED}  ❌ SOME CHECKS FAILED ($((ELAPSED/1000)).${ELAPSED: -3}s)${NC}"
    echo -e "${YELLOW}     Fix issues above and stage changes again.${NC}"
fi
echo -e "${CYAN}═══════════════════════════════════════════${NC}"

exit $EXIT_CODE
