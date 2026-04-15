---
name: jules
description: Jules is SWAL's autonomous coding agent from Google. Use via Jules CLI or GitHub issues with `jules` label. Jules works ONLY in iberi22/* repos. Updated 2026-04-15.
metadata:
  openclaw:
    emoji: "🤖"
    autoLoad: true
    requires:
      anyBins: ["jules"]
    triggers:
      - "jules"
      - "jules ai"
      - "agente asíncrono"
      - "agente asincrono"
      - "google agent"
      - "jules-ai"
---

# Jules Skill — Autonomous Coding Agent

> **⚡ SWAL Agent System — Updated 2026-04-15**

## ⚡ Quick Reference

```bash
# List ALL Jules sessions (PRIMARY command)
jules remote list --session

# List connected repos
jules remote list --repo

# Create new session
jules new --repo iberi22/<REPO> "task description"
jules new --repo iberi22/<REPO> --parallel 3 "task"

# Pull session result (view what Jules did)
jules remote pull --session <ID>

# Pull AND apply patch to local repo
jules remote pull --session <ID> --apply

# Teleport (clone repo + checkout + apply patch)
jules teleport <SESSION_ID>
```

---

## 1. Authentication

### Check if Logged In

```bash
jules remote list --repo
```
- Shows repos = ✅ Logged in
- Error "No valid client" = ❌ Not logged in

### Login (One-Time)

```bash
jules login
```
Opens browser for Google OAuth.

### Logout

```bash
jules logout
```

---

## 2. Jules CLI Commands

### Session Management

```bash
# List all sessions (MOST IMPORTANT)
jules remote list --session

# Show session details
jules remote pull --session <ID>

# Apply session patch to local repo
jules remote pull --session <ID> --apply

# Clone repo and apply session changes
jules teleport <SESSION_ID>

# Create new session
jules new --repo iberi22/<REPO> "task description"
```

### Session Status Values

| Status | Meaning | Action |
|--------|---------|--------|
| `In Progress` | Jules está trabajando | Esperar |
| `Awaiting User F` | Jules pidió input y espera | Revisar, extraer parcial, eliminar y relanzar |
| `Completed` | Listo para review | `jules remote pull --apply` |
| `Failed` | Error | Revisar, eliminar y recrear issue |

---

## 3. How to Assign Tasks to Jules

### Method 1: CLI (Recommended)

```bash
# Direct task assignment
jules new --repo iberi22/<REPO> "implement feature X"

# Multiple parallel sessions
jules new --repo iberi22/<REPO> --parallel 3 "write unit tests"
```

### Method 2: GitHub Issues + Label

**ONLY the `jules` label triggers Jules!**

```bash
# 1. Create issue
gh issue create --repo iberi22/<REPO> --title "[feat] Description" --body "Problem, solution, files"

# 2. Add jules label
gh issue edit <NUM> --repo iberi22/<REPO> --add-label jules

# 3. Jules picks it up automatically
```

### ⚠️ IMPORTANT: Label is REQUIRED

- `@jules-ai` mention = NOT a trigger
- Issue assignment = NOT a trigger
- **`jules` label** = ONLY trigger

---

## 4. When Sessions Get Stuck ("Awaiting User F")

**PROTOCOL:**

1. **Revisar** qué hizo Jules antes de detenerse:
   ```bash
   jules remote pull --session <ID>
   ```

2. **Extraer trabajo parcial** — código generado está en el output

3. **Eliminar sesión** — desde https://app.jules.ai o dejar que expire

4. **Analizar por qué se detuvo**:
   - ¿Contexto insuficiente?
   - ¿Dependencias no resueltas?
   - ¿Repo no limpio?

5. **Relanzar con mejor contexto**:
   ```bash
   jules new --repo iberi22/<REPO> "task + contexto adicional"
   ```

---

## 5. Session Status Monitoring

### Check All Sessions

```bash
jules remote list --session
```

### Filter by Status

| Filter | Command |
|--------|---------|
| In Progress | `jules remote list --session` (look for "In Progress") |
| Awaiting User F | `jules remote list --session` (look for "Awaiting User F") |
| Completed | `jules remote list --session` (look for "Completed") |
| Failed | `jules remote list --session` (look for "Failed") |

### Apply Completed Sessions

```bash
# Apply to local repo
jules remote pull --session <ID> --apply

# Or teleport (clone fresh)
jules teleport <SESSION_ID>
```

---

## 6. Jules in SWAL Projects

| Project | Repo | Status |
|---------|------|--------|
| OrionHealth | iberi22/OrionHealth | ✅ Active |
| Cortex | iberi22/cortex-1 | ✅ Active |
| Synapse Protocol | iberi22/synapse-protocol | ✅ Active |
| ManteniApp | iberi22/manteniapp | ✅ Active |
| WorldExams | iberi22/worldexams | ✅ Active |
| GOS | iberi22/gastronomic-os | ✅ Active |
| Gestalt-Rust | iberi22/gestalt-rust | ✅ Active |

> ⚠️ Jules ONLY works in `iberi22/*` repos.

---

## 7. Workflow: Assign Task → Monitor → Apply

```bash
# 1. ASSIGN
jules new --repo iberi22/OrionHealth "implement feature X"

# 2. MONITOR (check every few minutes)
jules remote list --session

# 3. When Completed, APPLY
jules remote pull --session <ID> --apply

# 4. Push to origin
git push origin develop
```

---

## 8. Jules + Gestalt Swarm — Combined Workflow

**Jules** handles complex async coding tasks (large refactors, new features, PR reviews).
**Gestalt Swarm** handles fast parallel CLI operations (scans, analysis, bulk operations).

### Combined Power Pattern

```
User: "Jules, analiza gestalt-rust para bugs y correcciones"
         ↓
Jules (async) → Planning + code review
Gestalt Swarm (sync) → Parallel exec: rg, cargo, git, curl
         ↓
Jules → Implement fixes based on Swarm results
         ↓
PR + Merge
```

### When to Use Each

| Task | Use |
|------|-----|
| Large refactors, new features | Jules (`jules new --repo iberi22/...`) |
| Bug finding, security audit | Gestalt Swarm (`swarm_bridge.py --goal "..."`) |
| Comprehensive analysis | Jules + Gestalt Swarm combined |
| Quick status checks | Gestalt Swarm only |
| Deep coding, PR review | Jules only |

### Gestalt Swarm Quick Commands

```bash
# Full analysis (smart agent selection)
python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "comprehensive security audit" --json

# Quick status
python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "git status" --agents "git_analyzer,git_status" --json --quiet

# Dry run (preview agents)
python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "analyze codebase" --dry-run

# Custom agents
python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "check dependencies" --agents "dep_check,cargo_check" --json

# Streaming mode
python E:\scripts-python\gestalt-rust\swarm_bridge.py --goal "scan files" --watch --timeout 30
```

### Workflow Integration

```bash
# 1. Run Gestalt Swarm for fast analysis
python swarm_bridge.py --goal "security audit" --max-agents 10 --json

# 2. Based on results, assign to Jules
jules new --repo iberi22/gestalt-rust "fix security issues found: [paste from swarm output]"

# 3. Monitor Jules
jules remote list --session

# 4. When complete, apply
jules remote pull --session <ID> --apply
```

---

## 9. Troubleshooting

### "Not logged in"
```bash
jules login
```

### Session stuck
```bash
# Check what's happening
jules remote pull --session <ID>

# If truly stuck (Awaiting User F), let it expire or delete from app.jules.ai
```

### Jules made wrong changes
1. Don't merge the PR
2. Create new issue with corrected instructions
3. Add `jules` label

### Session Failed
1. Review what went wrong
2. Fix the issue/instructions
3. Create new session with better context

---

## 10. Best Practices

1. **Repo must be clean** before sending to Jules
   ```bash
   git status  # must be "nothing to commit"
   git checkout develop
   ```

2. **Provide clear context** in the task description

3. **Break large tasks** into smaller ones (1 feature = 1 issue)

4. **Monitor sessions** — don't let them sit in "Awaiting User F"

5. **Apply completed sessions promptly** before they expire

6. **Use Gestalt Swarm for pre-flight checks** before assigning to Jules

7. **Combined workflow for maximum agentic power**: Swarm first, then Jules based on results

---

## 11. Available Agents in Gestalt Swarm

| Agent | For |
|-------|-----|
| `code_analyzer` | Ripgrep patterns in .rs files |
| `dep_check` | Cargo tree dependencies |
| `cargo_check` | Cargo check compilation |
| `git_analyzer` | Git log history |
| `git_status` | Working tree status |
| `file_scanner` | List files in directory |
| `log_parser` | Find ERROR in logs |
| `security_audit` | Find TODO/FIXME/unsafe |
| `find_todos` | Find TODO/FIXME/HACK |
| `api_tester` | Test HTTP endpoints |
| `metrics` | Cargo tree stats |
| `env_check` | Environment variables |
| `doc_gen` | Find documentation files |

---

*Last updated: 2026-04-15*
*Jules = Google's autonomous AI coding agent*
*Gestalt Swarm = SWAL's parallel execution bridge*
