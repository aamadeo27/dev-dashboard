# Epic 9 — Observability

Local structured logging and in-app health signals. No remote telemetry (NFR-8). Concrete schema lives in `docs/monitoring.md` — Coder must follow that doc field-for-field.

---

### T9.1 [backend] IPC instrumentation middleware

- **Description**: Add an `instrument(name, |cid| async { ... })` helper in `src-tauri/src/ipc/commands.rs` that wraps every `#[tauri::command]`. It generates a `correlation_id` (uuid v4), opens `info_span!("ipc", command, correlation_id)`, captures `elapsed_ms` on close, and on `Err(AppError)` emits `tracing::error!(kind, correlation_id, command, message, "command failed")` AND attaches `correlation_id` to `AppError::details` so the frontend toast can display it. Non-streaming commands additionally emit a `warn!("slow command", elapsed_ms)` when `elapsed_ms > 500` (exclude `launch_run`, `load_transcript`). See monitoring.md §1.3.e and §2.2.
- **Acceptance**:
  - Every `#[tauri::command]` body is `instrument("name", |cid| async move { ... }).await`.
  - Running any command produces a JSON log line with `span.command=<name>` and an `elapsed_ms` field on the close event.
  - Forcing an `AppError::NotFound` yields one `"command failed"` error log AND the frontend receives `details.correlation_id` matching the log.
  - `cargo test --test ipc_log_fields` asserts each expected field is present.
- **Dependencies**: T0.6, T0.2.

---

### T9.2 [backend] Run, transcript, and parser logging

- **Description**: Apply the exact field set from monitoring.md §2.3, §2.4, §2.5 to `runs/session.rs`, `runs/parser.rs`, `runs/transcript.rs`. Includes: `run started` (with `spawn_latency_ms`, `pid`, `cli_path`), `run finished` (with `status`, `exit_code`, `duration_ms`, `events_emitted`, `bytes_in`), `spawn failed` error, parse-warning lines with `line_no`/`bytes_dropped`/`snippet`, slow-flush warns over 100 ms, and `transcript opened` info. Per-batch parse spans use `info_span!("parse_batch", run_id, bytes_in)` and only emit at INFO when `elapsed_ms > 50`.
- **Acceptance**:
  - A test run produces `"run started"` and `"run finished"` log events with all required fields.
  - Feeding a malformed line to `EventParser` produces a `warn` log with `kind="parse_error"` and a 200-char snippet, and a `System` event is appended to the transcript.
  - Forcing an `io::Error` on transcript write produces a `error` log with `kind="io"` and propagates `AppError::Io`.
- **Dependencies**: T9.1, T4.1, T4.2, T4.3.

---

### T9.3 [backend] Background-task logging (git, usage, retention, orphan, sequence, registry, settings)

- **Description**: Apply monitoring.md §2.6 - §2.12 to `projects/git.rs`, `usage.rs`, `runs/retention.rs`, `runs/orphan.rs`, `sequences/mod.rs`, `projects/mod.rs`, `settings.rs`. Includes: `git_poll` span with `git_error_class` classification on failure, slow-poll warn over 1000 ms, `usage_probe` span with `keys_parsed` on success and stderr_tail on failure, `usage snapshot stale` warn rate-limited to once per minute, per-prune `"run pruned"` info lines with `reason=age|size`, orphan-reap summary + per-kill lines.
- **Acceptance**:
  - With a fake repo that errors out, the log shows a `warn` with `kind="git"`, `git_error_class="repo_missing"`.
  - With a 600 MB / 500 MB cap fixture (re-using T4.6 test), the retention summary log shows `runs_pruned > 0` and per-deletion lines.
  - With a mock CLI returning non-zero, `usage probe failed` log includes `exit_code` and a non-empty `stderr_tail`.
- **Dependencies**: T9.1, T2.3, T4.5, T4.6, T7.1.

---

### T9.4 [shared] Frontend error pipe and correlation_id surfacing

- **Description**: Add a React root error boundary in `src/App.tsx` that calls `log_frontend_error({ message, stack, route })`. Update the existing `log_frontend_error` backend handler to emit the exact log shape from monitoring.md §1.3.k (`component="frontend"`, `kind="frontend"`, generated `correlation_id`, included `route` and `stack`). Update `src/utils/errors.ts` (built in T8.1) so any `AppError` whose `details.correlation_id` is set renders a small "CID: 8 chars" chip in the toast body and in the run-failure-toast body. Add a "Copy" button on the chip.
- **Acceptance**:
  - Throwing inside a React component triggers exactly one `error` log line with `component="frontend"` and a stack.
  - A command that returns an `AppError` shows a toast whose body includes the CID chip; clicking Copy puts the full UUID on the clipboard.
  - The same CID appears in `dev-dashboard.<date>.log` under the original `"command failed"` line.
- **Dependencies**: T9.1, T6.1, T8.1.

---

### T9.5 [backend] EnvFilter, log retention sweep, and Settings tooltip

- **Description**: In `init_tracing()`, replace any static level with `EnvFilter::try_from_env("DEV_DASHBOARD_LOG").unwrap_or_else(|_| EnvFilter::new("info"))` so users can target a module (e.g. `info,dev_dashboard::runs::parser=debug`). On app startup (after subscriber init, before any business work), scan `<logs_dir>` and delete `dev-dashboard.*.log` files with mtime older than 7 days, logging a single `info` line with `removed` count and `bytes_freed`. Update the Settings screen (T1.4) "Open logs folder" area with a short tooltip / help text showing the env-var syntax.
- **Acceptance**:
  - `DEV_DASHBOARD_LOG=warn` results in INFO lines being filtered out (verified by integration test asserting absence).
  - `DEV_DASHBOARD_LOG="info,dev_dashboard::runs::parser=debug"` produces parser DEBUG lines while keeping other modules at INFO.
  - A logs dir pre-seeded with 10 daily files, half older than 7 days, results in only the recent half remaining after startup, plus a single summary log line.
  - Settings screen shows the tooltip text and links to the logs folder.
- **Dependencies**: T0.6, T1.4.
