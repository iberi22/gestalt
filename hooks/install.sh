#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────
# Gestalt Hooks Installer
# ──────────────────────────────────────────────────────────
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "🔧 Installing Gestalt Git Hooks..."
echo ""

# Ensure .git/hooks directory exists
mkdir -p ".git/hooks"

# Install pre-commit hook
cp "hooks/pre-commit.sh" ".git/hooks/pre-commit"
chmod +x ".git/hooks/pre-commit"

# Install commit-msg hook (embedded here)
cat > ".git/hooks/commit-msg" << 'HOOK'
#!/usr/bin/env bash
# Gestalt Commit Message Hook — Conventional Commits
set -euo pipefail
MSG_FILE="$1"
MSG=$(cat "$MSG_FILE")
if echo "$MSG" | grep -qE '^Merge '; then exit 0; fi
if ! echo "$MSG" | grep -qE '^(feat|fix|docs|style|refactor|perf|test|chore|ci|build|revert)(\([a-z0-9_-]+\))?!?: .+'; then
    echo ""
    echo "❌ Invalid commit message format!"
    echo "   Must follow: type(scope): description"
    echo "   Types: feat, fix, docs, style, refactor, perf, test, chore, ci, build, revert"
    echo "   Example: feat(router): implement WebSocket broadcast"
    echo ""
    echo "   Your message: $MSG"
    exit 1
fi
while IFS= read -r line; do
    if [ ${#line} -gt 72 ] && ! echo "$line" | grep -qE '^[#;]'; then
        echo "⚠️  Warning: Line >72 chars ($(echo "$line" | wc -c) chars)"
    fi
done < "$MSG_FILE"
exit 0
HOOK
chmod +x ".git/hooks/commit-msg"

# Install post-commit hook (Xavier indexing)
cp "hooks/post-commit.sh" ".git/hooks/post-commit"
chmod +x ".git/hooks/post-commit"

# Install pre-push hook (Gestalt validation)
cp "hooks/pre-push.sh" ".git/hooks/pre-push"
chmod +x ".git/hooks/pre-push"

echo "  ✅ pre-commit   — Rust format + clippy + build + tests + security"
echo "  ✅ commit-msg   — Conventional commit validation"
echo "  ✅ post-commit  — Index commit in Xavier via Gestalt"
echo "  ✅ pre-push     — Gestalt conflict detection"
echo ""
echo "✨ Hooks installed successfully!"
echo "   To skip hooks temporarily: git commit --no-verify"
echo "   Repo: $REPO_ROOT"
