# FASE 9: Sub-Agents CLI Integration - Pending Tasks

Generated: 2026-02-06
Priority: HIGH

## Tasks

---

### 9.1: Instalar CLI de Agentes

**Agent:** main
**Status:** Pending

**Objective:**
Install and configure all agent CLI tools.

**Implementation:**
```bash
#!/bin/bash
# install_agents.sh

echo "🚀 Installing Agent CLIs..."

# Jules CLI
echo "📦 Installing Jules CLI..."
npm install -g @jules/jules-cli

# Gemini CLI
echo "📦 Installing Gemini CLI..."
pip install google-gemini

# Qwen CLI
echo "📦 Installing Qwen CLI..."
pip install qwen-cli

# Codex CLI
echo "📦 Installing Codex CLI..."
npm install -g @openai/codex

# Omo CLI
echo "📦 Installing Omo CLI..."
npm install -g @omo/omo-cli

# Verify installations
echo "✅ Verifying installations..."
for cmd in jules gemini codex omo; do
    if command -v $cmd &> /dev/null; then
        echo "✅ $cmd: $(which $cmd)"
    else
        echo "⚠️ $cmd: not found"
    fi
done

echo "🎉 Agent CLIs installed!"
```

**Files to Create:**
- `scripts/install_agents.sh`
- `scripts/verify_agents.sh`

**Acceptance Criteria:**
- [ ] All 5 CLIs installed
- [ ] Commands available in PATH
- [ ] Version checks pass

---

### 9.2: Probar Integración con Agentes Reales

**Agent:** main
**Status:** Pending

**Objective:**
Test integration with each agent CLI.

**Implementation:**
```python
# test_agent_integration.py

import subprocess
import asyncio
from pathlib import Path

AGENTS = {
    "jules": {"cmd": "jules", "type": "github"},
    "gemini": {"cmd": "gemini", "type": "google"},
    "codex": {"cmd": "codex", "type": "openai"},
    "omo": {"cmd": "omo", "type": "ai"},
}

async def test_agent(agent_name: str) -> dict:
    """Test if agent is available and responding."""
    config = AGENTS[agent_name]
    
    try:
        # Simple version check
        result = await asyncio.create_subprocess_exec(
            config["cmd"], "--version",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )
        stdout, stderr = await result.communicate()
        
        return {
            "agent": agent_name,
            "type": config["type"],
            "status": "available" if result.returncode == 0 else "error",
            "version": stdout.decode().strip() or stderr.decode().strip()
        }
    except FileNotFoundError:
        return {
            "agent": agent_name,
            "type": config["type"],
            "status": "not_found",
            "version": None
        }

async def test_all_agents():
    """Test all agents."""
    results = await asyncio.gather(
        *[test_agent(name) for name in AGENTS]
    )
    
    for result in results:
        status_icon = "✅" if result["status"] == "available" else "⚠️"
        print(f"{status_icon} {result['agent']}: {result['status']}")
    
    return results
```

**Files to Create:**
- `test_agent_integration.py`
- `docs/agent-integration.md`

---

### 9.3: Crear Scripts de Automatización de Issues

**Agent:** main
**Status:** Pending

**Objective:**
Create scripts to automate GitHub issue creation from tasks.

**Implementation:**
```python
# issue_automation.py

import os
import json
from datetime import datetime
from pathlib import Path

class IssueAutomation:
    """Automate GitHub issue creation."""
    
    def __init__(self, repo: str, token: str = None):
        self.repo = repo
        self.token = token or os.getenv("GITHUB_TOKEN")
    
    def create_issue_from_task(self, task: dict) -> str:
        """Create GitHub issue from task definition."""
        title = task.get("title", "Untitled Task")
        body = self._format_task_body(task)
        labels = task.get("labels", ["task"])
        
        # Use gh CLI
        cmd = [
            "gh", "issue", "create",
            "--repo", self.repo,
            "--title", title,
            "--body", body,
            "--label", ",".join(labels)
        ]
        
        result = subprocess.run(cmd, capture_output=True)
        if result.returncode == 0:
            return result.stdout.strip()
        return None
    
    def _format_task_body(self, task: dict) -> str:
        """Format task as issue body."""
        lines = [
            f"## Task: {task.get('id', '?')}",
            "",
            f"**Description:** {task.get('description', '')}",
            f"**Phase:** {task.get('phase', 'unknown')}",
            f"**Priority:** {task.get('priority', 'medium')}",
            "",
            "### Implementation Notes",
            task.get("notes", "No notes provided."),
            "",
            f"_Created: {datetime.now().isoformat()}_"
        ]
        return "\n".join(lines)
    
    def bulk_create_from_file(self, file_path: str):
        """Create issues from task file."""
        with open(file_path) as f:
            tasks = json.load(f)
        
        created = []
        for task in tasks:
            url = self.create_issue_from_task(task)
            if url:
                created.append((task.get("id"), url))
        
        return created
```

**Files to Create:**
- `scripts/issue_automation.py`
- `scripts/create_issues_from_tasks.py`

---

### 9.4: Configurar Routing Automático de Tareas

**Agent:** main
**Status:** Pending

**Objective:**
Configure automatic task routing to appropriate agents.

**Implementation:**
```python
# task_router.py

from enum import Enum
from dataclasses import dataclass
from typing import List, Optional

class Agent(Enum):
    JULES = "jules"
    GEMINI = "gemini"
    CODEX = "codex"
    OMO = "omo"
    MAIN = "main"

@dataclass
class Task:
    id: str
    title: str
    description: str
    phase: str
    priority: str
    tags: List[str]

class TaskRouter:
    """Route tasks to appropriate agents based on characteristics."""
    
    ROUTING_RULES = {
        Agent.JULES: ["rust", "github", "workflow", "pr", "git"],
        Agent.GEMINI: ["docs", "documentation", "analysis", "report"],
        Agent.CODEX: ["python", "code", "implementation", "feature"],
        Agent.OMO: ["task", "schedule", "automation"],
    }
    
    def route(self, task: Task) -> Agent:
        """Determine best agent for task."""
        content = f"{task.title} {task.description} {','.join(task.tags)}".lower()
        
        scores = {}
        for agent, keywords in self.ROUTING_RULES.items():
            score = sum(1 for kw in keywords if kw in content)
            scores[agent] = score
        
        # Return highest scoring agent, default to MAIN
        if scores:
            best = max(scores, key=scores.get)
            if scores[best] > 0:
                return best
        
        return Agent.MAIN
    
    def route_and_assign(self, task: Task) -> dict:
        """Route task and create assignment."""
        agent = self.route(task)
        return {
            "task": task.id,
            "assigned_agent": agent.value,
            "reason": self._get_reason(task, agent),
            "estimated_time": self._estimate_time(task, agent)
        }
    
    def _get_reason(self, task: Task, agent: Agent) -> str:
        """Explain routing decision."""
        content = f"{task.title} {task.description}".lower()
        keywords = self.ROUTING_RULES.get(agent, [])
        matched = [kw for kw in keywords if kw in content]
        return f"Matched keywords: {matched}" if matched else "Default routing"
    
    def _estimate_time(self, task: Task, agent: Agent) -> str:
        """Estimate completion time."""
        estimates = {
            Agent.JULES: "2-4 hours",
            Agent.GEMINI: "1-2 hours",
            Agent.CODEX: "4-8 hours",
            Agent.OMO: "30 mins",
            Agent.MAIN: "Variable",
        }
        return estimates.get(agent, "Variable")
```

**Files to Create:**
- `scripts/task_router.py`

---

### 9.5: Tests de Integración Multi-Agente

**Agent:** main
**Status:** Pending

**Objective:**
Create integration tests for multi-agent system.

**Implementation:**
```python
# test_multi_agent.py

import pytest
from task_router import TaskRouter, Task, Agent

class TestTaskRouter:
    """Tests for task routing system."""
    
    def setup_method(self):
        self.router = TaskRouter()
    
    def test_rust_task_routes_to_jules(self):
        """Rust tasks should route to Jules."""
        task = Task(
            id="test-1",
            title="Implement Rust feature",
            description="Create a new Rust module",
            phase="FASE_3",
            priority="high",
            tags=["rust", "implementation"]
        )
        assert self.router.route(task) == Agent.JULES
    
    def test_docs_task_routes_to_gemini(self):
        """Documentation tasks should route to Gemini."""
        task = Task(
            id="test-2",
            title="Write API documentation",
            description="Create documentation for the API",
            phase="FASE_4",
            priority="medium",
            tags=["docs", "api"]
        )
        assert self.router.route(task) == Agent.GEMINI
    
    def test_python_task_routes_to_codex(self):
        """Python tasks should route to Codex."""
        task = Task(
            id="test-3",
            title="Add Python feature",
            description="Implement new Python function",
            phase="FASE_5",
            priority="high",
            tags=["python", "feature"]
        )
        assert self.router.route(task) == Agent.CODEX
    
    def test_unknown_task_routes_to_main(self):
        """Unmatched tasks should route to main."""
        task = Task(
            id="test-4",
            title="Some random task",
            description="Description without keywords",
            phase="FASE_1",
            priority="low",
            tags=[]
        )
        assert self.router.route(task) == Agent.MAIN

class TestAgentIntegration:
    """Tests for agent integration."""
    
    def test_all_agents_configured(self):
        """Verify all agents are configured."""
        agents = ["jules", "gemini", "codex", "omo"]
        for agent in agents:
            assert agent in Agent.__members__.values()
```

**Files to Create:**
- `tests/multi_agent/test_router.py`
- `tests/multi_agent/test_integration.py`

---

## Files to Create

```
scripts/
├── install_agents.sh
├── verify_agents.sh
├── issue_automation.py
├── task_router.py
└── create_issues_from_tasks.py

tests/multi_agent/
├── __init__.py
├── test_router.py
└── test_integration.py

docs/
├── agent-integration.md
└── task-routing.md
```

---

## Definition of Done

- [ ] All CLIs installed
- [ ] Integration tests pass
- [ ] Routing system working
- [ ] Automation scripts functional
- [ ] Documentation complete
