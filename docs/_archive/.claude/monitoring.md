# Monitoring Configuration: Dev Dashboard v1

**Date**: 2026-05-18
**Companion docs**: `docs/kb/system-design.md` §7 (direction), `requirements.md` (NFR-8: no remote telemetry)
**Audience**: Coder (implements instrumentation), developer-as-user (reads logs)

## 0. Scope

Dev Dashboard is a local-first Tauri 2 desktop app. "Monitoring" here means **structured local logs** the developer can `grep`/`jq`, **in-app health pills** for live state, and **self-diagnosis recipes**. **No external services. No outbound network. No telemetry.** (NFR-8)

- **Level**: Basic (per KB §7.1). All signals stay on disk in `<os_config_dir>/dev-dashboard/logs/`.
- **Tools**: `tracing` + `tracing-subscriber` (JSON layer) + `tracing-appender` (daily rotation, retain 7 files). Cost: zero.
- **No alerting**: there is no on-call; the developer is the operator. Health surfaces in the app UI (toasts + pills).

---

## 1. Log Schema

### 1.1 Common envelope

Every line is one JSON object produced by `tracing_subscriber::fmt::Layer::json()`. Fields the subscriber adds automatically (do NOT set by hand):

| Field | Source | Example |
|---|---|---|
| `timestamp` | subscriber (RFC3339 UTC) | `"2026-05-18T14:22:31.412Z"` |
| `level` | macro | `"INFO"`, `"WARN"`, `"ERROR"` |
| `target` | module path | `"dev_dashboard::runs::session"` |
| `fields.message` | first positional arg | `"run started"` |
| `span.name` + `span.<field>` | enclosing `info_span!` | see §1.3 |
| `threadId`, `threadName` | subscriber | — |

### 1.2 Required custom fields (set on the event or its enclosing span)

All times in milliseconds unless suffix says otherwise. Booleans lowercase. IDs are strings. Use snake_case for every field.

| Field | Type | Where it applies | Notes |
|---|---|---|---|
| `kind` | string | every `error!` and `warn!` | One of: `not_found`, `already_exists`, `io`, `git`, `cli`, `parse_error`, `invalid_input`, `permission_denied`, `internal`, `frontend`. Matches `AppError` variants snake-cased, plus `frontend` for forwarded errors. |
| `correlation_id` | string (uuid v4) | every `error!`; also on `toast:show` payloads tied to a failure | Generated at the boundary that creates the error (command entry, task body). Surfaced in toast body so user can grep. |
| `component` | string | every event | One of: `run_manager`, `run_session`, `event_parser`, `transcript_writer`, `git_poller`, `usage_probe`, `retention_pruner`, `orphan_reaper`, `sequence_loader`, `project_registry`, `settings_store`, `cli_detect`, `ipc`, `frontend`. |
| `run_id` | string | anything inside a `RunSession` or referencing a run | Same id used in `meta.json`. |
| `project_id` | string | anything project-scoped | Registry id. |
| `command` | string | IPC command spans | Snake_case command name (`launch_run`, `get_git_status`, …). |
| `elapsed_ms` | u64 | every timed span at close | Emitted by the `close` event of `info_span!` (subscriber config: `with_span_events(FmtSpanEvents::CLOSE)`). |
| `source` | string | `log_frontend_error` ingest only | Always `"frontend"`. |

### 1.3 Per-category schemas

All examples show the **fields** the code must attach via macro syntax. The wrapping JSON envelope (§1.1) is added by the subscriber.

#### a) Run lifecycle (component=`run_session`)

`info_span!("run", run_id, project_id, sequence_name)` wraps `RunSession::run()`.

- **run started** — `tracing::info!(spawn_latency_ms, cli_path, attached_md = ?attached_md_path, "run started")`
  - `spawn_latency_ms`: time from command entry to `Child` handle in hand
  - `cli_path`: resolved Claude CLI path
- **run finished** — `tracing::info!(status, exit_code, duration_ms, events_emitted, bytes_in, "run finished")`
  - `status`: `"completed" | "failed" | "stopped"`
  - `exit_code`: i32 (`-1` if killed)
  - `events_emitted`: count of `RunEvent`s written to transcript
  - `bytes_in`: total stdout+stderr bytes read
- **run spawn failed** — `tracing::error!(kind = "cli", correlation_id, exit_code, stderr_tail, "spawn failed")`
  - `stderr_tail`: last 512 bytes of stderr (if any)
- **stop requested** — `tracing::info!(reason, "stop requested")`
  - `reason`: `"user" | "shutdown" | "orphan_reaper"`

#### b) Event parsing (component=`event_parser`)

`info_span!("parse", run_id, mode)` where `mode` is `"heuristic"` (interactive mode only; stream-json removed in v1).

- **parse warning** — `tracing::warn!(kind = "parse_error", line_no, bytes_dropped, snippet, "malformed event")`
  - `line_no`: u64, line index within transcript
  - `bytes_dropped`: u32, length of malformed slice that became a `System` event
  - `snippet`: first 200 chars of the bad line
- **per-batch timing** (only if `elapsed_ms > 50`) — auto-emitted on span close via a dedicated `parse_batch` span: `info_span!("parse_batch", run_id, bytes_in)`.

#### c) Transcript writer (component=`transcript_writer`)

- **write failed** — `tracing::error!(kind = "io", correlation_id, run_id, file = "transcript.jsonl" | "raw.log" | "meta.json", io_kind, "transcript write failed")`
  - `io_kind`: `err.kind().to_string()` (e.g. `"PermissionDenied"`)
- **flush slow** — `tracing::warn!(run_id, elapsed_ms, file, "slow flush")` when a single flush takes > 100 ms
- **rotation note** (info, once per run start) — `tracing::info!(run_id, transcript_path, raw_log_path, "transcript opened")`

#### d) Git poller (component=`git_poller`)

`info_span!("git_poll", project_id)` wraps `poll_one`.

- **poll ok** — auto-close emits `elapsed_ms`. Add explicit `tracing::debug!(branch, is_clean, dirty_files, ahead, behind, "git status")` inside the span at `debug` level (off by default).
- **poll slow** — `tracing::warn!(project_id, elapsed_ms, "slow git poll")` when `elapsed_ms > 1000`
- **poll error** — `tracing::warn!(kind = "git", project_id, git_error_class, message, "git poll failed")`
  - `git_error_class`: `"repo_missing" | "permission_denied" | "not_a_repo" | "other"`

#### e) IPC commands (component=`ipc`)

A single middleware-style wrapper macro `command_span!(name)` produces `info_span!("ipc", command = name, correlation_id)` for every `#[tauri::command]`. On span close, the subscriber emits `elapsed_ms`.

- **command error** — `tracing::error!(kind, correlation_id, command, message, "command failed")` inside the wrapper when the inner returns `Err(AppError)`.
- **command slow** — `tracing::warn!(command, elapsed_ms, "slow command")` when `elapsed_ms > 500` (but only for non-streaming commands; exclude `launch_run`, `load_transcript`).

#### f) Subprocess lifecycle (component=`run_manager` or `usage_probe`)

- **spawn** — `tracing::info!(component, cli_path, args = ?args, cwd = ?cwd, pid, "subprocess spawned")`
- **exit** — `tracing::info!(component, pid, exit_code, duration_ms, "subprocess exited")`
- **kill** — `tracing::info!(component, pid, signal = "SIGTERM" | "TerminateProcess", "subprocess killed")`

#### g) Usage probe (component=`usage_probe`)

`info_span!("usage_probe")` wraps `UsageProbe::fetch`.

- **fetch ok** — `tracing::info!(keys_parsed, elapsed_ms, "usage fetched")`
- **fetch failed** — `tracing::warn!(kind = "cli", exit_code, stderr_tail, "usage probe failed")`
- **parse failed** — `tracing::warn!(kind = "parse_error", raw_len, "usage parse failed")`
- **snapshot stale** — `tracing::warn!(age_secs, "usage snapshot stale")` (emitted when `get_usage` returns an entry older than `2 * usage_poll_interval_secs`)

#### h) Retention pruner (component=`retention_pruner`)

`info_span!("retention_run", trigger)` where `trigger` is `"startup" | "daily"`.

- **summary** — `tracing::info!(runs_pruned, bytes_freed, projects_scanned, elapsed_ms, "retention summary")`
- **per-deletion** — `tracing::info!(project_id, run_id, reason, age_days, size_bytes, "run pruned")`
  - `reason`: `"age" | "size"`
- **prune error** — `tracing::warn!(kind = "io", project_id, run_id, message, "retention delete failed")`

#### i) Orphan reaper (component=`orphan_reaper`)

`info_span!("orphan_reap")` wraps the startup scan.

- **scan summary** — `tracing::info!(candidates, killed, ignored, elapsed_ms, "orphan scan done")`
- **per-kill** — `tracing::info!(project_id, run_id, pid, exe_match = true, "orphan killed")`
- **ignored** — `tracing::info!(project_id, run_id, pid, exe_match = false, reason, "orphan ignored")`
  - `reason`: `"exe_mismatch" | "pid_dead" | "pid_not_found"`

#### j) Settings + CLI detection

- **settings load** — `tracing::info!(component = "settings_store", path, "settings loaded")` or `tracing::warn!(component = "settings_store", kind = "parse_error", path, archived_to, "settings corrupt, defaults restored")`
- **settings save** — `tracing::info!(component = "settings_store", elapsed_ms, "settings saved")` or `tracing::error!(component = "settings_store", kind = "io", message, "settings save failed")`
- **cli detect ok** — `tracing::info!(component = "cli_detect", resolved_path, version, mode = "interactive", "cli detected")`
- **cli detect failed** — `tracing::warn!(component = "cli_detect", kind = "cli", path_tried, message, "cli detect failed")`
- **cli lost** — `tracing::warn!(component = "cli_detect", last_known_path, "cli lost mid-session")`

#### k) Frontend errors (component=`frontend`)

Forwarded via `log_frontend_error(message, stack, route?)`:

- `tracing::error!(component = "frontend", kind = "frontend", correlation_id, route, stack, "{message}")`

### 1.4 File and retention

- Path: `<os_config_dir>/dev-dashboard/logs/dev-dashboard.YYYY-MM-DD.log`
- Rotation: daily via `tracing_appender::rolling::daily`
- Retention: keep 7 files; on app startup, delete `dev-dashboard.*.log` older than 7 days
- Encoding: UTF-8, LF line endings (KB §6.4)
- Default level: `info`. Override via `DEV_DASHBOARD_LOG=debug|trace`. The env-var filter parses `tracing-subscriber::EnvFilter` directives, so `DEV_DASHBOARD_LOG="info,dev_dashboard::runs::parser=debug"` is supported and documented in Settings -> Open logs folder help text.

---

## 2. Instrumentation Points (Rust, per component)

Format: file path -> function -> exact macro calls. All field names match §1.

### 2.1 `src-tauri/src/main.rs` — startup

- `init_tracing()` (called first in `main`):
  - Build `EnvFilter::try_from_env("DEV_DASHBOARD_LOG").unwrap_or_else(|_| EnvFilter::new("info"))`
  - Build `tracing_appender::rolling::daily(logs_dir, "dev-dashboard.log")`
  - Build `tracing_subscriber::fmt::layer().json().with_current_span(true).with_span_events(FmtSpan::CLOSE)`
  - On boot, after subscriber is installed: `tracing::info!(version = env!("CARGO_PKG_VERSION"), os = std::env::consts::OS, log_dir = ?logs_dir, "app start")`
  - On graceful shutdown: `tracing::info!(active_runs, elapsed_ms, "app shutdown")`

### 2.2 `src-tauri/src/ipc/commands.rs` — IPC wrapper

Provide a single helper used by every command body:

```rust
async fn instrument<T, F, Fut>(name: &'static str, f: F) -> AppResult<T>
where F: FnOnce(Uuid) -> Fut, Fut: Future<Output = AppResult<T>>
```

- Generates `correlation_id = Uuid::new_v4()`
- Opens `let span = tracing::info_span!("ipc", command = name, correlation_id = %correlation_id);`
- Enters span via `.instrument(span).await`
- On `Err(e)`: `tracing::error!(kind = e.kind_str(), correlation_id = %correlation_id, command = name, message = %e, "command failed")` then attach `correlation_id` to the `AppError::details` JSON so the frontend toast displays it
- Span close emits `elapsed_ms` automatically; the wrapper additionally emits `warn!` if `elapsed_ms > 500` for non-streaming commands

Every `#[tauri::command]` body is `instrument("name", |cid| async move { ... }).await`.

### 2.3 `src-tauri/src/runs/session.rs` — `RunSession`

- `RunSession::start()`:
  - Open `let span = info_span!("run", run_id = %run_id, project_id = %project_id, sequence_name = %seq);`
  - Just after `Command::spawn`: `tracing::info!(spawn_latency_ms, cli_path = %cli, attached_md = ?attached, pid = child.id(), "run started")`
  - Spawn-fail branch: `tracing::error!(kind = "cli", correlation_id = %cid, exit_code = -1, stderr_tail, "spawn failed")` then return `AppError::Cli`
- `RunSession::on_exit(status)`:
  - `tracing::info!(status = %status_str, exit_code, duration_ms, events_emitted, bytes_in, "run finished")`
- `RunSession::request_stop(reason)`:
  - `tracing::info!(reason = %reason, "stop requested")`

### 2.4 `src-tauri/src/runs/parser.rs` — `EventParser`

- `EventParser::feed(&mut self, bytes)`:
  - Wrap call site (per batch) with `info_span!("parse_batch", run_id = %self.run_id, bytes_in = bytes.len())` — span close emits `elapsed_ms`; subscriber filter drops if `< 50` ms (custom layer drop OR rely on `if elapsed_ms > 50 { tracing::info!(...) }` inside Drop guard — Coder picks the simpler one)
- On malformed line:
  - `tracing::warn!(kind = "parse_error", component = "event_parser", run_id = %self.run_id, line_no = self.line_no, bytes_dropped = bad.len() as u32, snippet = %first_200(bad), "malformed event")`
  - Continue: emit a `RunEvent::System { text: format!("[malformed line @ {line_no}]") }` so the transcript records it

### 2.5 `src-tauri/src/runs/transcript.rs` — `TranscriptWriter`

- `TranscriptWriter::open(run_id, dir)`:
  - `tracing::info!(component = "transcript_writer", run_id = %run_id, transcript_path = ?p1, raw_log_path = ?p2, "transcript opened")`
- `write_event(&mut self, ev)`:
  - On `io::Error`: `tracing::error!(component = "transcript_writer", kind = "io", correlation_id = %self.correlation_id, run_id = %self.run_id, file = "transcript.jsonl", io_kind = %e.kind(), "transcript write failed")` then propagate `AppError::Io`
- `flush()`:
  - Measure elapsed; if `> 100`: `tracing::warn!(component = "transcript_writer", run_id = %self.run_id, elapsed_ms, file = "transcript.jsonl", "slow flush")`

### 2.6 `src-tauri/src/projects/git.rs` — `GitPoller`

- `poll_one(&self, project_id)`:
  - `let span = info_span!("git_poll", project_id = %project_id, component = "git_poller");`
  - On `git2::Error`: classify into `git_error_class` (match on `err.code()` and `err.class()`); emit `tracing::warn!(kind = "git", project_id = %project_id, git_error_class = %class, message = %err, "git poll failed")`
  - Slow branch: after span close, if `elapsed_ms > 1000`, emit `tracing::warn!(project_id = %project_id, elapsed_ms, "slow git poll")` (use a `tracing::Instrument` adapter that captures the duration via `Instant::now()` rather than reading span fields)

### 2.7 `src-tauri/src/usage.rs` — `UsageProbe`

- `fetch(&self)`:
  - `info_span!("usage_probe", component = "usage_probe")`
  - On success: `tracing::info!(keys_parsed = snapshot.parsed.len(), elapsed_ms, "usage fetched")`
  - Subprocess non-zero exit: `tracing::warn!(kind = "cli", component = "usage_probe", exit_code, stderr_tail, "usage probe failed")`
  - Parse failure (empty / unparseable stdout): `tracing::warn!(kind = "parse_error", component = "usage_probe", raw_len = stdout.len(), "usage parse failed")`
- `get_snapshot(&self)`:
  - If snapshot age `> 2 * usage_poll_interval_secs`: `tracing::warn!(component = "usage_probe", age_secs, "usage snapshot stale")` (rate-limited: at most once per minute)

### 2.8 `src-tauri/src/runs/retention.rs` — `RetentionPruner`

- `run(trigger)`:
  - `info_span!("retention_run", trigger = %trigger, component = "retention_pruner")`
  - For each deletion: `tracing::info!(project_id = %pid, run_id = %rid, reason = %r, age_days, size_bytes, "run pruned")`
  - Summary at end: `tracing::info!(runs_pruned, bytes_freed, projects_scanned, elapsed_ms, "retention summary")`
  - Delete failure (per-run): `tracing::warn!(kind = "io", project_id = %pid, run_id = %rid, message = %err, "retention delete failed")`

### 2.9 `src-tauri/src/runs/orphan.rs` — `OrphanReaper`

- `run()`:
  - `info_span!("orphan_reap", component = "orphan_reaper")`
  - Per candidate kept (killed): `tracing::info!(project_id, run_id, pid, exe_match = true, "orphan killed")`
  - Per candidate skipped: `tracing::info!(project_id, run_id, pid, exe_match = false, reason = %r, "orphan ignored")`
  - Summary: `tracing::info!(candidates, killed, ignored, elapsed_ms, "orphan scan done")`

### 2.10 `src-tauri/src/sequences/mod.rs` — `SequenceLoader`

- `load_all(project_id)`:
  - `info_span!("sequence_load", project_id, component = "sequence_loader")`
  - Per non-UTF8 file fallback: `tracing::warn!(kind = "parse_error", project_id, file, "sequence not utf8, lossy decoded")`

### 2.11 `src-tauri/src/projects/mod.rs` — `ProjectRegistry`

- `load()`: span `info_span!("registry_load", component = "project_registry")` -> emits `elapsed_ms` on close; if file corrupt: `tracing::warn!(kind = "parse_error", path, archived_to, "registry corrupt, restored empty")`
- `save()`: span emits `elapsed_ms`; on IO error: `tracing::error!(kind = "io", path, message, "registry save failed")`

### 2.12 `src-tauri/src/settings.rs` — `SettingsStore`

Same pattern as registry: `settings_load`, `settings_save` spans; warn on corrupt, error on save IO.

### 2.13 `src-tauri/src/ipc/commands.rs` — `verify_claude_cli`

- On success: `tracing::info!(component = "cli_detect", resolved_path, version, mode = "interactive", "cli detected")`
- On failure: `tracing::warn!(component = "cli_detect", kind = "cli", path_tried, message, "cli detect failed")`

### 2.14 `src-tauri/src/ipc/commands.rs` — `log_frontend_error`

- Always: `tracing::error!(component = "frontend", kind = "frontend", correlation_id = %Uuid::new_v4(), route = %route.unwrap_or("?"), stack = %stack.unwrap_or(""), "{message}")`

### 2.15 Window focus bridge

- `tracing::debug!(component = "window_focus", state = "focused" | "blurred", "window state change")` — debug-level only; useful when investigating "why aren't pollers running".

---

## 3. In-App Health Signals (UI surfaces)

Every signal has: (a) the user-visible UI element, (b) the source log event(s), (c) the trigger condition.

| UI surface | What it shows | Source signal | Trigger |
|---|---|---|---|
| **Rate-limit pill** (top bar, UI §5.2) | KV pairs from `claude /usage`, or `--` placeholder | `usage:updated` event (data) + `usage_probe` warn logs | `available=false` -> render `--`; popover shows "Last check failed" with the `correlation_id` from the most recent `usage probe failed` log |
| **CLI-lost banner** (top of dashboard) | "Claude CLI not found at <path>. Open Settings." | `cli:lost` event | Emitted by the 60s detect loop when transition `found -> not-found` |
| **Project card git error pill** | Red dot + tooltip "Git status unavailable: <class>" | `git:updated` event with `status.error = Some(...)`; backed by `git poll failed` warn | Last poll returned `git_error_class != "other"` for that project |
| **Project card missing state** (UI §5.2) | Greyed card, "Folder missing — Relocate?" | `project:missing` event | `path.exists() == false` at any registry list / refresh |
| **Run failure toast** (UI §5.9) | "Sequence X failed — open log" with `correlation_id` chip | `toast:show` emitted on `run:finished` with `status=failed`, body includes correlation_id from the spawn or parse error | `run:finished` with failure |
| **Transcript-unavailable state** (S-05) | Error card with [Open folder] | `load_transcript` AppError; backed by `transcript write failed` errors | `load_transcript` returns `ParseError` or `Io` |
| **Step-failure prompt card** (UI §5.4) | Inline Retry/Skip/Abort/Continue card | `run:step_failure` event | Parser emits `StepFailed` |
| **Settings -> "Open logs folder" button** | Reveals log dir in OS file manager | n/a | Always available |
| **Frontend error toast** (dev mode only) | Generic "Internal error (CID: …)" | `log_frontend_error` -> `tracing::error!(kind=frontend)` | React error boundary catches unhandled render error |

Rate-limiting in the UI: at most one CLI-lost banner; toasts already bounded to 4 visible (UI §5.9); a project that emits 10 git errors in a minute only shows one pill (not a flood).

---

## 4. Self-Diagnosis Guide

All recipes assume the logs folder is open (Settings -> Open logs folder). Suggested tools: `jq`, plain editor search. JSON-per-line means each log is a single object on one line.

### 4.1 Run fails immediately

1. Find the run id (toast body, or `meta.json`).
2. `jq -c 'select(.run_id == "<id>")' dev-dashboard.<date>.log`
3. Look for:
   - `"spawn failed"` with `kind=cli` and `stderr_tail` -> CLI rejected the args or the binary path is wrong.
   - `"run started"` followed by `"run finished"` with `status=failed` and tiny `duration_ms` (< 500) -> child exited; check `exit_code` and the `transcript.jsonl` for any `Error` events.
   - No matching entries at all -> the command wrapper recorded an `"command failed"` at the `ipc` span; search for the `correlation_id` shown in the toast.

### 4.2 Git status stops updating

1. `jq -c 'select(.component == "git_poller")' dev-dashboard.<date>.log | tail -n 50`
2. Look for:
   - Repeated `"git poll failed"` with `git_error_class=repo_missing` -> the project moved.
   - `"slow git poll"` repeating (`elapsed_ms` climbing) -> repo is huge or on a network share; bump `git_poll_interval_secs` in Settings.
   - **No log lines at all in the last minute** -> the poller is paused. Check `component=window_focus` at `debug` (re-run with `DEV_DASHBOARD_LOG=debug`); window blur stops polling.

### 4.3 Usage bar shows `--`

1. `jq -c 'select(.component == "usage_probe")' dev-dashboard.<date>.log | tail -n 20`
2. Look for:
   - `"usage probe failed"` with non-zero `exit_code` + `stderr_tail` -> CLI args or auth issue; reproduce with `claude /usage` in a shell.
   - `"usage parse failed"` with `raw_len > 0` -> CLI output shape changed; capture `raw.log` from a recent run for the Coder.
   - `"usage snapshot stale"` -> the probe scheduler stopped firing (likely window-focus issue, same as 4.2).

### 4.4 App startup slow

1. `jq -c 'select(.fields.message == "app start" or .fields.message == "orphan scan done" or .fields.message == "retention summary" or .fields.message == "registry loaded" or .fields.message == "settings loaded")' dev-dashboard.<date>.log | head -n 20`
2. The `elapsed_ms` on each tells you which startup phase dominates. Common culprits:
   - `orphan_reap` `elapsed_ms` high -> many registered projects with stale `meta.json` files.
   - `retention_run` `elapsed_ms` high on `trigger=startup` -> first run after a long period; subsequent starts will be fast.
   - `registry_load` slow -> projects file has grown or is on slow disk.

### 4.5 Transcript missing in history view

1. From the affected run, note `run_id`.
2. `jq -c 'select(.run_id == "<id>" and (.component == "transcript_writer" or .fields.message == "command failed"))' dev-dashboard.<date>.log`
3. Look for:
   - `"transcript write failed"` with `kind=io`, `io_kind=PermissionDenied` -> project dir lost write access; check OS permissions.
   - `"transcript write failed"` with `io_kind=NotFound` -> the `.claude/runs/<id>/` dir was deleted (likely by retention; cross-check with `"run pruned"` for the same `run_id`).
   - No transcript writer logs at all -> the writer never opened; correlate with `"spawn failed"` for the same run.

### 4.6 Generic recipes

- **Errors by kind (top 5)**:
  `jq -c 'select(.level == "ERROR") | .fields.kind' dev-dashboard.*.log | sort | uniq -c | sort -rn | head -n 5`
- **Slow commands**:
  `jq -c 'select(.fields.message == "slow command") | {command, elapsed_ms}' dev-dashboard.*.log`
- **One run, full trace**:
  `jq -c 'select(.run_id == "<id>")' dev-dashboard.*.log`
- **Follow a correlation_id through the call**:
  `jq -c 'select(.correlation_id == "<cid>")' dev-dashboard.*.log`

---

## 5. Setup Tasks

Concrete instrumentation tasks, sized for one Coder session each (~2–4 h). To be appended to `epics.md` as **Epic 9 — Observability**. Most depend on T0.6 (initial tracing scaffold).

| ID | Title | Scope |
|---|---|---|
| T9.1 | IPC instrumentation middleware | Implement the `instrument()` helper (§2.2). Wrap every existing `#[tauri::command]`. Include slow-command warn. |
| T9.2 | Run + transcript + parser logging | Apply §2.3, §2.4, §2.5 fields. Add per-batch parse span guarded by 50 ms threshold. Include event count + bytes accounting on `run finished`. |
| T9.3 | Background-task logging (git, usage, retention, orphan, sequence, registry, settings) | Apply §2.6 - §2.12 exactly. Include slow-poll warns and `snapshot stale` rate limiter. |
| T9.4 | Frontend error pipe + correlation_id surfacing | Implement React root error boundary -> `log_frontend_error`. Plumb `AppError.details.correlation_id` from Rust to the toast body (`utils/errors.ts` already exists per T8.1). |
| T9.5 | EnvFilter + log retention sweep | Wire `DEV_DASHBOARD_LOG` env var via `EnvFilter`. On startup, delete log files older than 7 days. Document env-var syntax on the Settings screen tooltip. |

Each task's acceptance criteria: a representative log line for every field listed in §1.3 appears in the file when the relevant code path runs (unit/integration tests assert presence of the JSON keys).
