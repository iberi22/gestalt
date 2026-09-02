# Gestalt Xavier Pipeline Architecture & Operations

## Overview
This document describes the operational procedures and features of the Gestalt-Xavier pipeline integration, with specific focus on cron job execution and provider model snapshot management.

---

## Cron Job Provider Snapshot & Auto-Pinning (`[GESTALT-PIPELINE-03]`)

### Problem & Context
When cron jobs are created or edited without explicitly pinning the LLM provider and model snapshots (`provider_snapshot` and `model_snapshot`), changes in default environments or unpinned edits can cause silent failures with status `drift_skip`.

Key affected jobs previously included:
- `a490 veeduria-wave1-integration`
- `be6fd gara-monitor`
- `092e hosteler-monitor`

### Solution & Behavior

1. **Auto-Pinning & Doctor Diagnostics via `hermes cron doctor`**:
   - `hermes cron doctor` inspects all registered cron jobs in `~/.hermes/cron/jobs.json`.
   - Identifies any jobs with unpinned (`null`) or drifted `provider_snapshot` / `model_snapshot`.

2. **Auto-Pin Fix Script (`periferia/scripts/cron-pin-fix.sh`)**:
   - Script located at `periferia/scripts/cron-pin-fix.sh`.
   - Runs `hermes cron doctor` to detect jobs with `drift_skip`.
   - Extracts drifted job IDs and invokes `hermes cron edit --id <job_id> --provider opencode-go --model muse-spark-1.2-contributor` to pin provider and model snapshots.
   - Configured to run hourly via Hermes cron:
     ```bash
     hermes cron create --schedule "every 1h" --script "periferia/scripts/cron-pin-fix.sh" --name "cron-monitor"
     ```

3. **Provider Flag Support on `hermes cron create`**:
   - `hermes cron create` accepts `--provider opencode-go` and `--model muse-spark-1.2-contributor` options to ensure new jobs are pinned upon creation.

### Verification Commands
- Check cron health:
  ```bash
  hermes cron doctor 2>&1 | grep -c "drift_skip"
  ```
- List cron jobs and verify zero drift:
  ```bash
  hermes cron list
  ```
- Help documentation check:
  ```bash
  hermes cron create --help
  ```
