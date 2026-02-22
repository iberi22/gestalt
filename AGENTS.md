---
title: "Git-Core Protocol - Agent Configuration"
type: CONFIGURATION
id: "config-agents"
created: 2025-12-20
updated: 2025-12-20
agent: copilot
model: gemini-3-pro
requested_by: system
summary: |
  Configuration rules for AI agents working on Gestalt.
keywords: [agents, rules, workflow, configuration]
tags: ["#configuration", "#agents", "#rules"]
project: Gestalt
protocol_version: 3.5.0
---

# 🤖 AGENTS.md - AI Agent Configuration

## 🎯 Prime Directive: Token Economy
```
Your state is GitHub Issues. Not memory. Not files. GitHub Issues.
```

## 🛡️ Architecture Verification Rule (MANDATORY)
**BEFORE implementing ANY infrastructure/tooling:**
1. Read `.gitcore/ARCHITECTURE.md` CRITICAL DECISIONS section
2. Verify your implementation matches the decided stack
3. If issue mentions alternatives, ARCHITECTURE.md decision wins

## 🔄 The Loop (Workflow)

### Phase 0: HEALTH CHECK
```bash
# 1. Check project state
git log --oneline -5
# 2. Run tests (if applicable to current scope)
cargo test -p gestalt_core
```

### Phase 1: READ (Context Loading)
```bash
# 1. Architecture
cat .gitcore/ARCHITECTURE.md
# 2. Current Task
gh issue list --assignee "@me"
```

### Phase 2: ACT (Development)
```bash
# Create feature branch
git checkout -b feat/issue-<ID>
# Implement & Commit
git commit -m "feat(scope): description (closes #<ID>)"
```

### Phase 3: UPDATE
```bash
# Push & PR
git push -u origin HEAD
gh pr create --fill
```

## ⛔ FORBIDDEN FILES (in Root)
- ❌ `TODO.md`, `TASKS.md` (Moved to `.gitcore/planning/`)
- ❌ `PLANNING.md` (Moved to `.gitcore/planning/`)
- ❌ `NOTES.md` (Use Issue Comments)

## ✅ ALLOWED FILES
- Source code (`.rs`, `.dart`)
- `.gitcore/ARCHITECTURE.md`
- `.gitcore/planning/` (Historical Reference)
- `.gitcore/sprints/` (Sprint progress)
- `.github/issues/*.md` (File-based issues)

## 🚀 Proactive Execution
**"No sugerir, HACER"**
- If user describes a bug -> Create Issue -> Fix -> PR
- If user wants a feature -> Create Issue -> Implement -> PR

---
*Aligned with Git-Core Protocol v3.5*
