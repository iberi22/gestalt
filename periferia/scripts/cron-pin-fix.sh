#!/usr/bin/env bash
set -euo pipefail

# periferia/scripts/cron-pin-fix.sh
# Auto-pin provider & model snapshots for Hermes cron jobs to prevent drift_skip errors.

echo "Running hermes cron doctor check..."
DOCTOR_OUTPUT=$(hermes cron doctor 2>&1 || true)
echo "$DOCTOR_OUTPUT"

if echo "$DOCTOR_OUTPUT" | grep -q "drift_skip"; then
    echo "Drift detected in cron jobs! Applying auto-fix via hermes cron edit..."
    # Extract job IDs reporting drift and run hermes cron edit on each
    DRIFT_JOBS=$(echo "$DOCTOR_OUTPUT" | grep "drift_skip" | sed -n 's/.*job \([a-zA-Z0-9_-]*\):.*/\1/p')
    if [ -z "$DRIFT_JOBS" ]; then
        # Fallback: run hermes cron edit across jobs
        hermes cron edit --provider opencode-go --model muse-spark-1.2-contributor
    else
        for job_id in $DRIFT_JOBS; do
            echo "Pinning job $job_id..."
            hermes cron edit --id "$job_id" --provider opencode-go --model muse-spark-1.2-contributor
        done
    fi
    echo "Cron pin fix applied successfully."
else
    echo "No cron drift detected. All jobs properly pinned."
fi

# Final verification with hermes cron doctor
echo "Re-running hermes cron doctor verification..."
hermes cron doctor

# Verify status post-fix
echo "Verifying hermes cron list status..."
hermes cron list
