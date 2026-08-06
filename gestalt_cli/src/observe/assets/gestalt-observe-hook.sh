#!/bin/sh
# Gestalt Observation Hook
# Does POST to http://127.0.0.1:8081/api/event with 10s timeout, fail-open, exit 0

AGENT="${1:-unknown-agent}"
EVENT_TYPE="${2:-run_started}"
SUMMARY="${3:-Agent execution event}"

# Fail-open POST using curl with --max-time 10 and silent failure
if command -v curl >/dev/null 2>&1; then
  curl -s -X POST \
    -H "Content-Type: application/json" \
    -d "{\"agent\":\"$AGENT\",\"event_type\":\"$EVENT_TYPE\",\"summary\":\"$SUMMARY\"}" \
    --max-time 10 \
    http://127.0.0.1:8081/api/event >/dev/null 2>&1 || :
fi

exit 0
