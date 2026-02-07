# FASE 9: Sub-Agents CLI Integration - IN PROGRESS

Generated: 2026-02-06
Priority: HIGH
**Status: IN PROGRESS**

## Tasks

---

### 9.1: Instalar CLI de Agentes

**Agent:** main
**Status:** IN PROGRESS

**Objective:**
Install and configure all agent CLI tools (jules, gemini, qwen, codex, omo).

**Files Created:**
- `scripts/install_agents.py`

**Implementation:**
```bash
python scripts/install_agents.py
```

**Agents:**
- Jules (GitHub automation)
- Gemini (Google AI)
- Qwen (Alibaba AI)
- Codex (OpenAI)
- Omo (Task automation)

**Acceptance Criteria:**
- [ ] All 5 CLIs installed
- [ ] Commands available in PATH
- [ ] Version checks pass

---

### 9.2: Probar Integración con Agentes Reales

**Agent:** main
**Status:** IN PROGRESS

**Objective:**
Test integration with each agent CLI.

**Files Created:**
- `scripts/test_agents.py`

**Tests:**
```bash
python scripts/test_agents.py
```

**Checks:**
- Command availability
- Version detection
- Path verification

**Acceptance Criteria:**
- [ ] All agents tested
- [ ] Report generated
- [ ] Integration verified

---

### 9.3: Scripts de Automatización de Issues

**Agent:** main
**Status:** PENDING

**Objective:**
Create scripts to automate GitHub issue creation from task templates.

**Files to Create:**
- `scripts/create_issue.py`
- `scripts/templates/task_template.md`

**Implementation:**
```python
python scripts/create_issue.py --phase FASE_5 --id 5.2 --title "Create Tests"
```

---

### 9.4: Routing Automático de Tareas

**Agent:** main
**Status:** IN PROGRESS

**Objective:**
Configure automatic task routing to appropriate agents.

**Files Created:**
- `scripts/task_router.py`

**Routing Rules:**
| Agent | Keywords |
|-------|----------|
| Jules | rust, github, workflow, pr, git, cicd |
| Gemini | docs, documentation, analysis, report |
| Codex | python, code, implementation, feature |
| Omo | schedule, cron, reminder, task |

**Usage:**
```python
from scripts.task_router import TaskRouter, Task

router = TaskRouter()
task = Task(id="9.1", title="Install CLI", description="Install Jules CLI")
assignment = router.route_and_assign(task)
print(assignment["assigned_agent"])  # "jules"
```

**Acceptance Criteria:**
- [ ] Routing working
- [ ] Statistics tracked
- [ ] Confidence scores

---

### 9.5: Tests de Integración Multi-Agente

**Agent:** main
**Status:** PENDING

**Objective:**
Create integration tests for multi-agent system.

**Files to Create:**
- `tests/test_multi_agent.py`

**Tests:**
- Task routing tests
- Agent availability tests
- Integration tests

**Acceptance Criteria:**
- [ ] All tests pass
- [ ] Coverage > 70%

---

## Files Created

```
scripts/
├── install_agents.py      # Install all agent CLIs
├── test_agents.py        # Test agent integration
├── task_router.py        # Automatic task routing
└── templates/
    └── TASK_TEMPLATES.md # Issue templates
```

---

## CI/CD Integration

```yaml
# .github/workflows/agents.yml
name: Agent Tests

on:
  schedule:
    - cron: '0 */4 * * *'  # Every 4 hours

jobs:
  test-agents:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install agents
        run: python scripts/install_agents.py
      
      - name: Test agents
        run: python scripts/test_agents.py
      
      - name: Route tasks
        run: python scripts/task_router.py
```

---

## Definition of Done

- [ ] All CLIs installed
- [ ] Integration tests passing
- [ ] Task routing working
- [ ] Documentation complete
- [ ] CI/CD configured
