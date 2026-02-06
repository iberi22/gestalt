# FASE 2: Obsidian Advanced - Pending Tasks

Generated: 2026-02-06
Priority: MEDIUM

## Tasks

---

### 2.6: Dataview Queries Pre-configuradas

**Agent:** main
**Status:** Pending

**Objective:**
Create pre-configured Dataview queries for common operations.

**Implementation:**
```markdown
<!-- Dataview Queries for Obsidian -->

## 📊 Project Tasks

### Pending Tasks
```dataview
TASK
FROM "1 Proyectos"
WHERE !completed
SORT file.name ASC
```

### Completed Tasks (This Week)
```dataview
TASK
FROM "1 Proyectos"
WHERE completed AND completed >= date(today) - dur(7 days)
SORT completed DESC
```

### Tasks by Priority
```dataview
TASK
FROM "1 Proyectos"
WHERE !completed AND !priority = null
GROUP BY priority
SORT priority DESC
```

---

## 📈 Project Progress

### All Projects Status
```dataview
TABLE project-status, progress, last-updated
FROM "1 Proyectos"
WHERE project = "gestalt-rust"
SORT file.name ASC
```

### GitHub Issues Integration
```dataview
TABLE issue-number, title, status
FROM "1 Proyectos/gestalt-rust"
WHERE issue-number
SORT issue-number DESC
```

---

## 🔍 Search Queries

### Recent Memories
```dataview
TABLE date, tags
FROM "0 Inbox"
WHERE date >= date(today) - dur(7 days)
SORT date DESC
LIMIT 20
```

### OpenAI/Gemini Notes
```dataview
LIST
FROM "3 Recursos"
WHERE contains(tags, "ai") OR contains(tags, "llm")
SORT file.name ASC
```

---

## Files to Create

```
1 Proyectos/
├── _TEMPLATES/
│   └── 📊 Dataview Queries.md
└── 0 Indices/
    └── DASHBOARD.md
```

---

### 📊 Dataview Queries.md Template

```markdown
---
tags: template, dataview
---

# 📊 Dataview Query Collection

## Task Queries

### Pending Tasks
\`\`\`dataview
TASK
FROM [[]]
WHERE !completed
SORT file.name ASC
\`\`\`

### Completed This Week
\`\`\`dataview
TASK
FROM [[]]
WHERE completed AND completed >= date(today) - dur(7 days)
SORT completed DESC
\`\`\`

## Project Queries

### All Tasks
\`\`\`dataview
TASK
FROM [[]]
WHERE project
GROUP BY project
\`\`\`

## Notes Queries

### Recent Notes
\`\`\`dataview
TABLE date
FROM [[]]
WHERE date
SORT date DESC
LIMIT 10
\`\`\`

---
```

---

## Definition of Done

- [ ] Dataview queries created
- [ ] Templates added to Obsidian
- [ ] Queries tested and working
- [ ] Documentation added

---

### 2.7: Configurar Graph View Connections

**Agent:** main
**Status:** Pending

**Objective:**
Configure Obsidian Graph View with project connections.

**Implementation:**
```json
{
  // Obsidian Graph Settings (JSON format for copy/paste)
  "graph": {
    "filters": {
      "includeTags": ["project", "task", "ai"],
      "excludeTags": ["template", "draft"]
    },
    "display": {
      "showTags": true,
      "showAttachments": false,
      "showArrows": true,
      "groupByFolder": false
    },
    "collapseFilter": false,
    "depthLimit": 3,
    "centroidFocus": true
  },
  
  // Suggested connections based on project structure
  "connections": {
    "gestalt-rust": [
      "synapse-agentic",
      "clawnode",
      "openclaw"
    ],
    "clawnode": [
      "openclaw",
      "telegram"
    ]
  }
}
```

**Files to Create:**
- `.obsidian/graph.json` (Obsidian graph configuration)
- `docs/graph-view-guide.md`

---

### 2.8: Webhooks para Cambios Automáticos

**Agent:** main
**Status:** Pending

**Objective:**
Implement webhook system to trigger actions on file changes.

**Implementation:**
```python
# obsidian_webhook.py

import os
import hashlib
import hmac
import json
from datetime import datetime
from pathlib import Path

class ObsidianWebhook:
    """Handle Obsidian file change webhooks."""
    
    def __init__(self, secret: str = None):
        self.secret = secret or os.getenv("OBSIDIAN_WEBHOOK_SECRET")
        self.handlers = []
    
    def register_handler(self, handler: callable):
        """Register a webhook handler."""
        self.handlers.append(handler)
    
    async def handle_change(self, file_path: str, change_type: str):
        """Process a file change."""
        payload = {
            "file": file_path,
            "type": change_type,
            "timestamp": datetime.now().isoformat()
        }
        
        # Verify signature if secret set
        if self.secret:
            payload["signature"] = self._sign(payload)
        
        # Execute all handlers
        for handler in self.handlers:
            await handler(payload)
    
    def _sign(self, payload: dict) -> str:
        """Create HMAC signature."""
        data = json.dumps(payload, sort_keys=True)
        return hmac.new(
            self.secret.encode(),
            data.encode(),
            hashlib.sha256
        ).hexdigest()
```

**Files to Create:**
- `obsidian_webhook.py`
- `.github/workflows/obsidian-sync.yml` (GitHub Actions)

---

## Files to Create

```
obsidian/
├── .obsidian/
│   └── graph.json
├── 📊 Dataview Queries.md
├── docs/
│   └── graph-view-guide.md
└── obsidian_webhook.py
.github/workflows/
└── obsidian-sync.yml
```

---

## Definition of Done

- [ ] Dataview queries working
- [ ] Graph view configured
- [ ] Webhooks implemented
- [ ] CI/CD integration working
