# Epic 4 — Run Execution Core

The heart. Unblocks live and historical run views.

---

### T4.1 [backend] EventParser

- **Description**: Stateful heuristic streaming parser for interactive Claude CLI stdout. Accepts byte chunks; emits `RunEvent`s using the pattern set documented in KB §6.7. Buffers partial lines. Patterns must be validated against actual Claude CLI output before this task is considered complete (see KB §9 item 1).
- **Acceptance**:
  - Unit tests cover: single chunk one event; one event split across many chunks; multiple events in one chunk; unknown line -> `AssistantText` fallback; malformed block body -> `system` event with raw bytes.
  - First parsed event is emitted within 250ms of receiving the first CLI output byte (NFR-5).
  - Patterns are loaded from `runs/parser/patterns.rs` with a logged version constant at run start.
  - Parser is allocation-conscious (no per-byte allocs).
- **Dependencies**: T0.2.

---

### T4.2 [backend] TranscriptWriter

- **Description**: Per-run writer task. Owns three file handles: `meta.json`, `transcript.jsonl`, `raw.log`. Atomic `meta.json` updates (tmp + rename). JSONL appended with a flush per line. Raw bytes appended to `raw.log` unchanged.
- **Acceptance**:
  - Killing the process mid-run leaves a valid (but partial) `transcript.jsonl` (no truncated line) and a `meta.json` that still parses.
  - Concurrent writers for different runs do not interfere.
- **Dependencies**: T0.2.

---

### T4.3 [backend] RunSession + RunManager

- **Description**: `RunManager` spawns Claude CLI as a child process with `cwd=project.path`. Manages a map of `run_id -> RunSession`. `RunSession` owns the child, parser, writer, cancellation token. Emits `run:started`, `run:event`, `run:finished`.
- **Acceptance**:
  - `launch_run` returns a `Run` with `status=Pending` within 200ms (NFR-4 visible feedback); `running` shortly after.
  - Stdin write via `send_input` reaches the child (validated with an echo binary in test).
  - Stop via `stop_run` kills the child, drains remaining output, finalizes meta.json with `status=stopped`.
  - Two simultaneous runs in the same project do not collide on transcript files (separate run-id dirs).
- **Dependencies**: T4.1, T4.2, T1.2.

---

### T4.4 [backend] Attached-md context handling

- **Description**: When `attached_md_path` is set on `LaunchInput`, prepend the file's content to the first stdin write (or as a CLI arg if Claude CLI supports it — Coder picks based on T1.2 probe). Record the path in `meta.json`.
- **Acceptance**:
  - File contents reach the child process.
  - Missing file at launch time -> `AppError::NotFound` before spawn.
- **Dependencies**: T4.3.

---

### T4.5 [backend] OrphanReaper

- **Description**: On app startup, scan all registered projects' `.claude/runs/*/meta.json`. For any with `status in (pending, running)`, check if PID is alive AND its exe path matches the configured claude CLI. If yes, kill it. Mark all such runs `failed` with `note="Terminated (app restarted)"`.
- **Acceptance**:
  - Smoke test: launch a run, force-quit the app, relaunch -> run is marked failed and the (mock) child is gone.
  - Conservative: a PID alive but with a different exe is NOT killed.
- **Dependencies**: T4.3, T2.1.

---

### T4.6 [backend] RetentionPruner

- **Description**: At startup and on a 24h timer, walk each project's runs dir. Apply both retention rules from settings (age days; total-size MB per project). Delete oldest first to satisfy both. Skip runs in any non-terminal state. Emit `info` log lines per deletion.
- **Acceptance**:
  - With 600MB of fake runs and a 500MB cap, oldest are pruned to bring it under.
  - With 31-day-old runs and a 30-day cap, those are pruned.
  - Active runs are never deleted.
- **Dependencies**: T4.3, T1.1.

---

### T4.7 [backend] respond_to_step_failure command + step-failure detection

- **Description**: Parser emits `StepFailed` event on detecting a step failure marker (markers per CLI integration — Coder pins during T4.1). `RunSession` also emits `run:step_failure`. `respond_to_step_failure(run_id, choice)` writes the appropriate token to stdin. A 60s timer auto-Continues if no response.
- **Acceptance**:
  - Mock CLI that emits a step-failure marker triggers the event.
  - Each of the four choices is dispatched per the protocol resolved in T4.8 (either via stdin token or via kill + re-invoke).
  - No response within 60s -> Continue is auto-sent and logged.
- **Dependencies**: T4.3, T4.8.

---

### T4.8 [backend] Research: step-failure interaction protocol

- **Description**: Before implementing Retry/Skip/Abort/Continue UI, verify whether Claude CLI (interactive mode) supports receiving specific stdin tokens to influence mid-run behavior, or whether "retry" must be implemented as: kill current subprocess + re-invoke the same step. Document findings in knowledge-base.md §9 and update T4.7 scope accordingly.
- **Acceptance**:
  - knowledge-base.md §9 updated with concrete finding: either (a) exact stdin tokens confirmed, or (b) "retry = new subprocess" approach documented with implementation plan.
  - T4.7 acceptance criteria updated to match.
- **Dependencies**: T4.1, T4.3 (need a running interactive session to test).
- **Note**: If stdin tokens don't work, the "Continue" action = re-invoke the step. "Retry" = same. "Skip" = advance to next step index. "Abort" = kill and mark failed. These can all be implemented without special stdin tokens.
