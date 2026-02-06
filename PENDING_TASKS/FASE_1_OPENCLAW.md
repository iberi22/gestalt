# FASE 1: OpenClaw Integration - Pending Tasks

Generated: 2026-02-06
Priority: MEDIUM

## Tasks

---

### 1.5: Probar integración completa con OpenClaw real

**Agent:** main
**Status:** Pending

**Objective:**
Test the complete integration with running OpenClaw instance.

**Implementation:**
```bash
# Test script
#!/bin/bash

echo "Testing OpenClaw Integration..."

# Test 1: Memory system initialization
python -c "from skills.openclaw_memory import initialize; initialize()"
echo "✅ Memory system initialized"

# Test 2: Search functionality
python -c "
from skills.openclaw_memory import memory_search
results = memory_search('test', limit=5)
print(f'✅ Search returned {len(results)} results')
"

# Test 3: Add memory
python -c "
from skills.openclaw_memory import add_memory
mem_id = add_memory('Integration test', 'test')
print(f'✅ Memory added: {mem_id}')
"

# Test 4: Get memory
python -c "
from skills.openclaw_memory import get_memory
memory = get_memory('test-id')
print(f'✅ Memory retrieved')
"

echo "✅ All integration tests passed!"
```

**Files to Create:**
- `test_openclaw_integration.sh`

**Acceptance Criteria:**
- [ ] All 4 tests pass
- [ ] No errors in logs
- [ ] Performance acceptable (< 1s per operation)

---

### 1.6: Modificar config de OpenClaw

**Agent:** main
**Status:** Pending

**Objective:**
Update OpenClaw configuration to enable memory system.

**Files to Modify:**
- `config/openclaw.yaml` (create if not exists)

**Configuration:**
```yaml
# openclaw.yaml

# Memory System Configuration
memory:
  enabled: true
  provider: openclaw
  cache_size: 10000
  ttl_hours: 24
  
# Search Configuration
search:
  default_limit: 10
  max_limit: 100
  similarity_threshold: 0.7
  
# Performance
performance:
  batch_size: 32
  parallel_workers: 4
  cache_enabled: true
```

**Implementation:**
```python
# skills/openclaw_memory.py (additions)

import yaml
from pathlib import Path

def load_config(config_path: str = "config/openclaw.yaml") -> dict:
    """Load OpenClaw configuration."""
    path = Path(config_path)
    if path.exists():
        with open(path) as f:
            return yaml.safe_load(f)
    return {}

def update_openclaw_config():
    """Update OpenClaw config with memory settings."""
    config = load_config()
    
    config["memory"] = {
        "enabled": True,
        "provider": "openclaw",
        "cache_size": 10000,
        "ttl_hours": 24
    }
    
    config["search"] = {
        "default_limit": 10,
        "max_limit": 100,
        "similarity_threshold": 0.7
    }
    
    # Save config
    with open("config/openclaw.yaml", "w") as f:
        yaml.dump(config, f)
    
    return config
```

**Acceptance Criteria:**
- [ ] Config file created/updated
- [ ] Settings load correctly
- [ ] Memory system uses config

---

## Files to Create

```
config/
└── openclaw.yaml

tests/
└── integration/
    ├── test_openclaw_integration.sh
    └── test_openclaw_integration.py
```

---

## Definition of Done

- [ ] Integration tests pass
- [ ] Configuration file created
- [ ] Settings load correctly
- [ ] Documentation updated
