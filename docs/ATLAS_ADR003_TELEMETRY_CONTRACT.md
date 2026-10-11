# Atlas ADR-003: Headless JSON Telemetry Contract

## 1. Overview

This normative document specifies the machine-checkable telemetry contract for the Gestalt headless agent JSON event emitter. This definition conforms to the Atlas observer pattern for headless agent telemetry.

**Traceability:**
- Feature Tracking Issue: #645
- Implementation Emitter Pull Request: #646 (pending PR #646)

## 2. Transport and Framing Rules

- **NDJSON Framing:** The output MUST be formatted as Newline Delimited JSON (NDJSON) over standard stream interfaces.
- **Line Rule:** Exactly one complete, valid JSON object per line emitted on `stdout`.
- **Whitespace:** No pretty-printing (no internal line breaks). The file MUST end with a trailing newline. Blank lines are STRICTLY PROHIBITED.
- **Producer Commands:** The exact producer command shape for generating events is documented as `gestalt run --telemetry-json` (implementation lands via PR #646).

## 3. Base Standard Fields

Every valid event emitted MUST contain the following root-level fields:

| Field            | Type   | Description                                                                                      |
|------------------|--------|--------------------------------------------------------------------------------------------------|
| `event`          | string | The discrete event type. MUST be one of: `StepProgress`, `ToolInvoked`, or `ErrorCaptured`.        |
| `schema_version` | string | The contract schema version. MUST be exactly `"1.0"`. (No other versions are permitted).         |
| `ts`             | string | The event timestamp. MUST strictly follow RFC3339 format in UTC (e.g., `2026-10-11T15:04:05Z`). |
| `run_id`         | string | A unique correlation identifier (e.g. UUIDv4 or KSUID) tying events to a specific execution run.   |

## 4. Normative Event Definitions

The schema strictly defines three event types. Unknown event types are invalid.

### 4.1. `StepProgress`

Emitted when the agent progresses to a new sequential phase or step of its execution cycle.

| Field         | Status   | Type    | Format / Notes                                                           |
|---------------|----------|---------|--------------------------------------------------------------------------|
| `step_name`   | Required | string  | Identifier of the current phase (e.g., `planning`, `tool_execution`).    |
| `status`      | Required | string  | The status of the step (e.g., `started`, `completed`, `skipped`).          |
| `duration_ms` | Optional | integer | Explicit elapsed time in milliseconds for the step.                      |

### 4.2. `ToolInvoked`

Emitted immediately when the agent invokes an external command, integration, or MCP tool.

| Field       | Status   | Type    | Format / Notes                                                           |
|-------------|----------|---------|--------------------------------------------------------------------------|
| `tool_name` | Required | string  | The exact string identifier of the invoked tool (e.g., `grep`, `bash`).  |
| `args`      | Required | object  | A JSON object containing key-value pairs representing tool arguments.    |
| `exit_code` | Optional | integer | The numeric exit code returned by the process, if applicable.            |

### 4.3. `ErrorCaptured`

Emitted when the agent intercepts a fault, exception, or non-zero exit that impacts execution.

| Field         | Status   | Type    | Format / Notes                                                           |
|---------------|----------|---------|--------------------------------------------------------------------------|
| `error_code`  | Required | string  | Categorical error identifier (e.g., `E_TOOL_TIMEOUT`, `E_SYNTAX`).       |
| `message`     | Required | string  | Developer-readable string detailing the exact error context.             |
| `fatal`       | Required | boolean | `true` if the fault triggers a hard crash, `false` if recoverable.       |
| `stack_trace` | Optional | string  | Raw stack trace string, if propagated by the runtime.                    |

## 5. Machine Conformance Rules

The following structural rules MUST be statically validated by any conformance harness:

1. `Rule 1 (NDJSON Integrity):` Every line MUST parse successfully as a valid JSON object without exceptions.
2. `Rule 2 (Base Correlation):` Every parsed object MUST include the exact keys: `event`, `schema_version`, `ts`, and `run_id`.
3. `Rule 3 (Strict Versioning):` The `schema_version` key MUST explicitly equal `"1.0"`.
4. `Rule 4 (Timestamp Format):` The `ts` key MUST correctly parse as an RFC3339 UTC timestamp.
5. `Rule 5 (Framing Hygiene):` The stream or file must conclude with exactly one trailing newline, containing zero blank lines between payloads.

## 6. Backward Compatibility Rules

When modifying schemas in the future, adhere to standard telemetry schema versioning principles:
- **Additive Changes:** New optional fields can be added without bumping the schema version.
- **Breaking Changes:** Removing or renaming a required field requires a new schema version. In this contract, only `1.0` is currently valid.

## 7. Delivery Instructions

For developers implementing this, any new tools should follow these structural rules and emit exactly in this machine-parsable format.
