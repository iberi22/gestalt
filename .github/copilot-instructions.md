# 🧠 GitHub Copilot Instructions - Gestalt

> **"⚡ Context-Aware, Multi-Model, Production-Ready"**

## Prime Directive
You are operating under the **Git-Core Protocol**. Your state is GitHub Issues, not internal memory.

## 🚀 Quick Commands

| Prompt | Description |
|--------|-------------|
| `#prompt:context` | 🆕 **Load Context** (Architecture + Issues) |
| `#prompt:issue` | Create a new issue |
| `#prompt:status` | Check project status |

## ⛔ FORBIDDEN ACTIONS (HARD RULES)

**NEVER create these files:**
- ❌ `TODO.md`, `TASKS.md`, `BACKLOG.md`
- ❌ `PLANNING.md`, `ROADMAP.md`
- ❌ `NOTES.md`, `SCRATCH.md`

**ALWAYS use:**
- ✅ `.github/issues/TYPE_description.md` for tasks
- ✅ `gh issue comment` for updates

## 🎯 Intent Detection - Issue Creation Flow

When user says "I need X" or "Fix Y":

1. **ANNOUNCE**: "Voy a crear un issue para trackear esto."
2. **CREATE FILE**: `.github/issues/{TYPE}_{short-desc}.md`
3. **SHOW**: "✅ Issue creado: ..."

## 🔄 The Loop (Workflow)

1. **READ**: `.gitcore/ARCHITECTURE.md` + `gh issue list --assignee @me`
2. **ACT**: `git checkout -b feat/issue-N` -> Code -> Test
3. **UPDATE**: `git commit` -> `gh pr create`

## 🏗️ Architecture First
Before implementing infrastructure:
1. Read `.gitcore/ARCHITECTURE.md`
2. If conflict, ARCHITECTURE wins.

## ⚛️ Atomic Commits
One logical change per commit.
Format: `type(scope): description #issue`

## 🚀 Non-Blocking Execution
Always run long commands (builds, tests) in background or redirect to file.
`cargo test > test_output.txt 2>&1`
