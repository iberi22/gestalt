#!/usr/bin/env bash
set -uo pipefail

# Default to the committed examples file
FILE="docs/ATLAS_ADR003_EXAMPLES.ndjson"

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --file) FILE="$2"; shift ;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

if [[ ! -f "$FILE" ]]; then
    echo "FAIL: File not found: $FILE"
    exit 1
fi

# The python3 script will validate the NDJSON file against the contract
python3 -c "
import json
import sys
import re
from datetime import datetime

file_path = '$FILE'
try:
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
except Exception as e:
    print(f'FAIL: Could not read file: {e}')
    sys.exit(1)

# Check for trailing newline
if not content.endswith('\n'):
    print('FAIL: File does not end with a trailing newline.')
    sys.exit(1)

# Check for blank lines
lines = content.split('\n')
# The last line should be empty because of the trailing newline
if lines[-1] != '':
    print('FAIL: Trailing newline missing or extra content after newline.')
    sys.exit(1)

lines = lines[:-1]
if any(line == '' for line in lines):
    print('FAIL: File contains blank lines.')
    sys.exit(1)

# Regex for RFC3339 UTC timestamp
rfc3339_regex = re.compile(r'^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$')

for i, line in enumerate(lines):
    try:
        obj = json.loads(line)
    except json.JSONDecodeError as e:
        print(f'FAIL: Line {i+1} is not valid JSON. Error: {e}')
        sys.exit(1)

    required_keys = {'event', 'schema_version', 'ts', 'run_id'}
    if not required_keys.issubset(obj.keys()):
        missing = required_keys - obj.keys()
        print(f'FAIL: Line {i+1} missing required base keys: {missing}')
        sys.exit(1)

    if obj['schema_version'] != '1.0':
        print(f'FAIL: Line {i+1} schema_version is not \"1.0\": {obj[\"schema_version\"]}')
        sys.exit(1)

    if not isinstance(obj['run_id'], str) or not obj['run_id']:
        print(f'FAIL: Line {i+1} run_id is invalid.')
        sys.exit(1)

    if not rfc3339_regex.match(obj['ts']):
        print(f'FAIL: Line {i+1} ts is not a valid RFC3339 UTC timestamp: {obj[\"ts\"]}')
        sys.exit(1)

    event_type = obj['event']
    if event_type == 'StepProgress':
        if 'step_name' not in obj or not isinstance(obj['step_name'], str):
            print(f'FAIL: Line {i+1} StepProgress missing valid step_name')
            sys.exit(1)
        if 'status' not in obj or not isinstance(obj['status'], str):
            print(f'FAIL: Line {i+1} StepProgress missing valid status')
            sys.exit(1)
    elif event_type == 'ToolInvoked':
        if 'tool_name' not in obj or not isinstance(obj['tool_name'], str):
            print(f'FAIL: Line {i+1} ToolInvoked missing valid tool_name')
            sys.exit(1)
        if 'args' not in obj or not isinstance(obj['args'], dict):
            print(f'FAIL: Line {i+1} ToolInvoked missing valid args object')
            sys.exit(1)
    elif event_type == 'ErrorCaptured':
        if 'error_code' not in obj or not isinstance(obj['error_code'], str):
            print(f'FAIL: Line {i+1} ErrorCaptured missing valid error_code')
            sys.exit(1)
        if 'message' not in obj or not isinstance(obj['message'], str):
            print(f'FAIL: Line {i+1} ErrorCaptured missing valid message')
            sys.exit(1)
        if 'fatal' not in obj or not isinstance(obj['fatal'], bool):
            print(f'FAIL: Line {i+1} ErrorCaptured missing valid fatal boolean')
            sys.exit(1)
    else:
        print(f'FAIL: Line {i+1} has unknown event type: {event_type}')
        sys.exit(1)

print('PASS: File perfectly respects the ADR-003 JSON telemetry contract.')
sys.exit(0)
"

exit $?
