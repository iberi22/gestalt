#!/usr/bin/env bash
set -euo pipefail
cd /home/belal/proyectosSWAL/gestalt

echo "=== CMD 1 ==="
git log -1 --format='%H %s %ci' 28e9209

echo "=== CMD 2 ==="
git show 28e9209 --stat --oneline | head -80

echo "=== CMD 3 ==="
git merge-base --is-ancestor 28e9209 HEAD; echo exit:$?

echo "=== CMD 4 ==="
git ls-tree -r --name-only 28e9209 -- scripts/swarm_bridge.py gestalt_core/src/adapters/persistence/surreal_db.rs gestalt_core/src/db/surreal.rs gestalt_core/src/mcp/mod.rs gestalt-router/src/router.rs gestalt-router/src/timeline.rs gestalt_cli/src/main.rs gestalt-ws/src/server.rs gestalt-state/src/statedb.rs .gitcore/SRC.md 2>&1

echo "=== CMD 5 ==="
git grep -n 'BeliefGraph\|tantivy\|BM25\|AtomicCheckpointer\|StateDbEventLog\|CliAdapter\|select_agents\|SurrealDbAdapter' 28e9209 -- '*.rs' '*.py' 2>&1 | head -80
