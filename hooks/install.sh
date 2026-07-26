#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt Hooks Installer
# ──────────────────────────────────────────────────────────
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "🔧 Installing Gestalt Git Hooks..."
echo ""

# Install pre-commit hook
cp ".git/hooks/pre-commit" ".git/hooks/pre-commit"
chmod +x ".git/hooks/pre-commit"

# Create commit-msg hook
cat > ".git/hooks/commit-msg" << 'HOOK'
#!/usr/bin/env bash
# ── Commit Message Linter ──────────────────────────────
set -euo pipefail

MSG_FILE="$1"
MSG=$(cat "$MSG_FILE")

# Conventional commit format: type(scope): description
if ! echo "$MSG" | grep -qE '^(feat|fix|docs|style|refactor|perf|test|chore|ci|build|revert)(\([a-z0-9_-]+\))?!?: .+'; then
    echo ""
    echo "❌ Invalid commit message format!"
    echo "   Must follow: type(scope): description"
    echo "   Types: feat, fix, docs, style, refactor, perf, test, chore, ci, build, revert"
    echo "   Example: feat(router): implement WebSocket broadcast"
    echo ""
    exit 1
fi

# Max line length
if echo "$MSG" | grep -q '^[^#]' | while IFS= read -r line; do
    [ ${#line} -gt 72 ] && exit 1
done; then
    echo ""
    echo "⚠️  Warning: Commit message line exceeds 72 characters"
    echo ""
fi
HOOK
chmod +x ".git/hooks/commit-msg"

echo "  ✅ pre-commit  — Rust format + clippy + build + tests + security"
echo "  ✅ commit-msg  — Conventional commit validation"
echo ""
echo "✨ Hooks installed successfully!"
echo "   To skip hooks temporarily: git commit --no-verify"
