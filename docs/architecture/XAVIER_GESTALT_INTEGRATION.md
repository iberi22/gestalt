# Xavier ↔ Gestalt Integration Architecture

## Overview
This document specifies the integration architecture between **Gestalt Agent Execution Runtime** and **Xavier RTK Proxy** for sub-agent tool execution and memory lifecycle management.

---

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                   Gestalt Xavier Cycle Runner                    │
│                 (scripts/gestalt-xavier-cycle.sh)                │
└────────────────┬────────────────────────────────┬────────────────┘
                 │                                │
    [1] PRE Query│                                │[4] POST Archival
                 ▼                                ▼
┌────────────────────────────────┐  ┌──────────────────────────────┐
│         Xavier Memory          │  │        Xavier Memory         │
│     POST /v1/memories/search   │  │       POST /v1/memories      │
└────────────────────────────────┘  └──────────────────────────────┘
                 │
   [2] Context Feed
                 ▼
┌──────────────────────────────────────────────────────────────────┐
│                   [3] Agent Command Execution                    │
│                                                                  │
│    Is `xavier` in PATH && HTTP 200 /health ?                     │
│         │                                                        │
│   ──────┴──────────────────────────┐                             │
│   │ YES                            │ NO                          │
│   ▼                                ▼                             │
│ Route via Xavier RTK Proxy     Fallback Direct Execution         │
│ `xavier exec "<cmd>"` or       `sh -c "<cmd>"`                   │
│ `xavier_run_command "<cmd>"`                                     │
│                                                                  │
│ Token Savings Telemetry:       Token Savings Telemetry:          │
│ mode=rtk_proxy                 mode=direct_fallback              │
│ tokens_saved_est=~N            tokens_saved_est=0              │
└──────────────────────────────────────────────────────────────────┘
```

---

## Command Execution Proxy (`xavier exec`)

To prevent verbose agent subshell outputs (`git diff`, `cargo test`, `npm test`) from consuming unnecessary context tokens in multi-agent runs, sub-agent tool runs in `gestalt-xavier-cycle.sh` are routed through the Xavier RTK Proxy:

1. **Proxy Discovery & Health Check:**
   - Checks if `xavier` binary is available in `PATH` and verifies HTTP endpoint health at `$XAVIER_URL/health`.
   - Alternatively checks for the MCP tool helper `xavier_run_command`.

2. **Execution Routing:**
   - **Primary Route:** `xavier exec "<cmd>"` / `xavier_run_command "<cmd>"`
     - Output is compressed and filtered through RTK (Reduce Token Knowledge) proxy rules.
     - Telemetry emitted: `[xavier-proxy] mode=rtk_proxy tokens_saved_est=<N> exit_code=<code_val>`
   - **Fallback Route:** `sh -c "<cmd>"`
     - Executed when Xavier is offline or `xavier` is not installed.
     - Telemetry emitted: `[xavier-proxy] mode=direct_fallback tokens_saved_est=0 exit_code=<code_val>`

---

## Archival & Telemetry Lifecycle

Execution results and `[xavier-proxy]` token savings telemetry are captured in Phase 3 and stored back in Xavier during Phase 4:

- **Path:** `gestalt/cycle/<timestamp>`
- **Kind:** `execution`
- **Metadata Fields:**
  - `source`: `"gestalt-cli-cycle"`
  - `query`: Goal or task prompt
  - `agent_cmd`: Sub-agent command executed
  - `xavier_proxy_telemetry`: Telemetry string containing mode, token savings estimate, and exit code
  - `timestamp`: ISO-8601 execution timestamp

---

## Verification

To verify syntax and fallback behavior:

```bash
# Check syntax
bash -n scripts/gestalt-xavier-cycle.sh

# Verify proxy invocation reference
grep -rn "xavier exec" scripts/gestalt-xavier-cycle.sh

# Test fallback mode
./scripts/gestalt-xavier-cycle.sh "test query" --agent "echo hello"
```
