## Objective
Return partial results as agents complete, instead of waiting for all to finish.

## Current Behavior
- Wait for ALL agents to complete
- Return full JSON only at the end
- Total time = slowest agent

## Desired Behavior
- Stream agent results as they complete
- Show progress: "agent_1 done (30ms), agent_2 running..."
- Return partial JSON with `completed` and `pending` agents

## Options

### Option 1: File polling (recommended)
```bash
python swarm_bridge.py --goal "..." --watch --output /tmp/swarm_{id}.json
# Polls /tmp/swarm_{id}.json every 100ms
```

### Option 2: WebSocket
- Requires separate server process
- More complex, higher overhead

### Option 3: SSE (Server-Sent Events)
- Similar complexity to WebSocket
- One-way streaming only

## Acceptance Criteria
- [ ] `--watch` flag in bridge
- [ ] Output file written incrementally
- [ ] `--watch` mode returns partial JSON with `completed_count`
- [ ] Polling from skill with configurable timeout

## Priority
**LOW** — Current parallel execution is fast enough for most use cases.
