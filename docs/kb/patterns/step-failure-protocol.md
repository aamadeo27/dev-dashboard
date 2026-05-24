# Step failure protocol

When the parser detects a step failure (`StepFailed` event):
1. Emit `run:event` with the `StepFailed` payload (UI renders inline).
2. Emit `run:step_failure` (UI surfaces the Retry/Skip/Abort/Continue prompt).
3. Wait for `respond_to_step_failure` command (with 60s default timeout -> auto-Continue).
4. Translate choice to stdin input expected by Claude CLI (exact tokens TBD by Coder during CLI integration).
