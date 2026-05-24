# Step failure protocol

**Resolved: option (b) kill + re-invoke** (T4.8, 2026-05-24).

No stdin token protocol was found in the Claude CLI. All four choices are implemented via process kill and optional re-invocation.

## Event flow

When the parser detects a step failure (`StepFailed` event):

1. Emit `run:event` with the `StepFailed` payload (UI renders inline).
2. Emit `run:step_failure` (UI surfaces the Retry / Skip / Abort / Continue prompt).
3. Start a 60 s countdown. If no `respond_to_step_failure` command arrives, auto-Continue.
4. On `respond_to_step_failure(run_id, choice)`, apply the per-choice action below.

## Per-choice actions

| Choice | Action |
|---|---|
| **Continue** | Write `"\n"` to child stdin. If child produces no new output within 2 s, kill + re-invoke with the original `LaunchInput`. Auto-triggered after the 60 s timeout; logged at `info` level. |
| **Retry** | `child.kill()`, then re-invoke Claude CLI with the identical `LaunchInput`. New `run_id` minted; `meta.json` records `retry_of: <original_run_id>`. |
| **Skip** | `child.kill()`, re-invoke with prompt prefixed `"Skip the previous failing step and continue."` + original prompt text. |
| **Abort** | `child.kill()`, mark run `RunStatus::Failed`, `exit_note = "Aborted by user"`. No re-invoke. |

## Rationale

T1.2 probed only `claude --version`; no step-failure interactive behavior was captured. The `EventParser` has `STEP_FAILED_SENTINEL = ""` (disabled), and `RunSession` has no step-failure-specific stdin routing. With no evidence of a stdin token protocol, option (b) is the correct conservative default: it works regardless of whether a token protocol exists and can be enhanced later if the CLI adds one.
