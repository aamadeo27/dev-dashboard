# Monitoring Configuration: Dev Dashboard v1

**Date**: 2026-05-18 (relocated/audited 2026-06-13 during project adoption)
**Companion docs**: `docs/kb/system-design.md` §7 (direction), `docs/requirements.md` (NFR-8: no remote telemetry)
**Audience**: Coder (implements instrumentation), developer-as-user (reads logs)

> This is the canonical monitoring doc. It is a faithful relocation of the original
> `.claude/monitoring.md` spec, plus a **Current instrumentation status** subsection
> per component (added during adoption) recording what exists in the Rust code today
> vs. what is still pending. Pending work is tracked by **Epic 9 — Observability**
> (`docs/epics/epic-9-observability/`, T9.1–T9.5). Inferred items are marked
> `> [adoption-assumption] …`.

## 0. Scope

Dev Dashboard is a local-first Tauri 2 desktop app. "Monitoring" here means **structured local logs** the developer can `grep`/`jq`, **in-app health pills** for live state, and **self-diagnosis recipes**. **No external services. No outbound network. No telemetry.** (NFR-8)

- **Level**: Basic (per KB §7.1). All signals stay on disk in `<os_config_dir>/dev-dashboard/logs/`.
- **Tools**: `tracing` + `tracing-subscriber` (JSON layer) + `tracing-appender` (daily rotation, retain 7 files). Cost: zero.
- **No alerting**: there is no on-call; the developer is the operator. Health surfaces in the app UI (toasts + pills).

### 0.1 Audit summary (adoption, 2026-06-13)

Audited against the actual Rust core on branch `feat/T4.7-step-failure-command` (Epic 4 in progress; Epics 2 & 3 done; Epic 7 not started).

| Component / area | Status | Notes |
|---|---|---|
| Tracing init (`logging.rs`) | **PARTIAL** | JSON layer + `EnvFilter(DEV_DASHBOARD_LOG)` + daily rolling all present. Missing: `FmtSpan::CLOSE` (no automatic `elapsed_ms`), 7-file/7-day retention sweep, the `app start`/`app shutdown` boot lines. File stem is `app` not `dev-dashboard`. |
| `AppError` kind field (`error.rs`) | **PARTIAL** | Has `code()` → SCREAMING_SNAKE. **No `kind_str()`** snake-case method that the `kind` log field requires. `details` hardcoded `null` (no `correlation_id` plumbing). |
| IPC wrapper (`ipc/commands.rs`) | **PENDING** | **No `instrument()` / `command_span!` wrapper.** No per-command `correlation_id`, no `ipc` span, no slow-command warn, no central `command failed` error log. Commands emit ad-hoc `tracing::info!` lines only. |
| Run lifecycle (`runs/session.rs`) | **PARTIAL** | `run_session` span + `run started` / `run finished` present, but missing `spawn_latency_ms`, `cli_path`, `events_emitted`, `bytes_in`, `duration_ms`; `status`/`exit_code` use Debug formatting, no `kind` on the spawn-fail path. |
| Event parser (`runs/parser.rs`) | **PARTIAL** | Has init line. **No `parse_batch` span, no `elapsed_ms`, no `parse warning` (`kind=parse_error`, `line_no`, `bytes_dropped`, `snippet`).** Malformed lines DO fall back to a `System` event (transcript record kept). |
| Transcript writer (`runs/transcript.rs`) | **PENDING** | **Zero tracing.** No `transcript opened`, no `transcript write failed` (`kind=io`), no slow-flush warn. |
| Git poller (`projects/git.rs`) | **PARTIAL** | Window-focus `debug!` present. **No `git_poll` span, no `elapsed_ms`, no slow-poll warn, no `git poll failed` warn with `kind=git` + `git_error_class`.** Errors are captured into `GitStatus.error` (sanitized string) and surfaced via the `git:updated` event, but never logged as a structured warn. |
| Usage probe (`usage/mod.rs`) | **PENDING** | Module is a stub (`// UsageProbe goes here`). Epic 7 not built — all §1.3g signals pending. |
| Retention pruner (`runs/retention.rs`) | **IMPLEMENTED (mostly)** | `retention_pruner` component, per-deletion `run pruned` with `reason=age|size`, summary, and prune-error warns all present. Gap vs spec: prune-error warns omit `kind="io"`; no `retention_run` named span / `trigger` field. |
| Orphan reaper (`runs/orphan.rs`) | **IMPLEMENTED (mostly)** | `orphan_reaper` component, start/found/kill/skip/finished lines present with `run_id`, `project_id`, `pid`, exe-match reasoning. Gap vs spec: field/message names differ (`orphan reaper finished` vs `orphan scan done`; `killed`/`marked` vs `candidates`/`killed`/`ignored`); no `orphan_reap` named span; no `elapsed_ms`. |
| Sequence loader (`sequences/mod.rs`) | **IMPLEMENTED (mostly)** | `sequence_loader` component + `load_all` span + warns present. Gap: non-utf8 fallback warn omits `kind="parse_error"`. |
| Project registry (`projects/mod.rs`) | **IMPLEMENTED (mostly)** | `project_registry` component on load/save + corrupt/IO warns present. Gap: corrupt-file warn omits `kind="parse_error"`; IO-error path is `warn` not `error` and omits `kind="io"`; no named `registry_load`/`registry_save` spans with `elapsed_ms`. |
| Settings store (`settings/mod.rs`) | **IMPLEMENTED (mostly)** | `settings loaded` / `settings saved` / corrupt-archive warns present. Gap: missing `component="settings_store"` on most lines, corrupt warn omits `kind="parse_error"`, save-IO path not an `error!` with `kind="io"`. |
| CLI detection (`verify_claude_cli`) | **IMPLEMENTED** | `cli detected` info + `cli detect failed` warn with `component="cli_detect"`, `kind="cli"`, `path_tried`, `message` — matches §2.13. |
| Frontend error pipe (`log_frontend_error`) | **PARTIAL** | Backend handler matches §1.3k (`component="frontend"`, `kind="frontend"`, generated `correlation_id`, `route`, `stack`, sanitized). **Pending: the React root error boundary** that calls it, and the toast CID chip (T9.4). |
| `cli:lost` mid-session signal | **PENDING** | No `cli lost` warn found in the CLI detect/watch loop. |

**Net:** the *foundational* tracing scaffold and several background components are wired up, but the three load-bearing pieces of the spec — the **IPC `instrument()` wrapper** (T9.1), the **`AppError` `kind_str()` + `correlation_id` plumbing** (T9.1/T9.4), and the **`FmtSpan::CLOSE` → `elapsed_ms` + retention sweep** (T9.5) — are not yet implemented. The `kind` log field exists in only two call sites (cli_detect, frontend). Epic 9 (T9.1–T9.5) covers all of this; see §6.

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

> [adoption-assumption] The current subscriber sets `.with_current_span(true).with_span_list(true)`,
> so span fields are emitted under `span` / `spans`. It does **not** set `FmtSpan::CLOSE`, so the
> automatic `elapsed_ms` close events described below are not yet produced (T9.5).

### 1.2 Required custom fields (set on the event or its enclosing span)

All times in milliseconds unless suffix says otherwise. Booleans lowercase. IDs are strings. Use snake_case for every field.

| Field | Type | Where it applies | Notes |
|---|---|---|---|
| `kind` | string | every `error!` and `warn!` | One of: `not_found`, `already_exists`, `io`, `git`, `cli`, `parse_error`, `invalid_input`, `permission_denied`, `internal`, `frontend`. Matches `AppError` variants snake-cased, plus `frontend` for forwarded errors. **Status: currently present only on `cli_detect` and `frontend` lines; T9.1–T9.3 add it everywhere else. Requires a new `AppError::kind_str()` (`error.rs` today only has `code()`).** |
| `correlation_id` | string (uuid v4) | every `error!`; also on `toast:show` payloads tied to a failure | Generated at the boundary that creates the error (command entry, task body). Surfaced in toast body so user can grep. **Status: present only in `log_frontend_error`; per-command generation is T9.1.** |
| `component` | string | every event | One of: `run_manager`, `run_session`, `event_parser`, `transcript_writer`, `git_poller`, `usage_probe`, `retention_pruner`, `orphan_reaper`, `sequence_loader`, `project_registry`, `settings_store`, `cli_detect`, `ipc`, `frontend`. **Status: present on most background components; absent on parser, transcript, run_session events, and the IPC layer.** |
| `run_id` | string | anything inside a `RunSession` or referencing a run | Same id used in `meta.json`. **Status: present on session/orphan/retention lines.** |
| `project_id` | string | anything project-scoped | Registry id. **Status: present where applicable.** |
| `command` | string | IPC command spans | Snake_case command name (`launch_run`, `get_git_status`, …). **Status: PENDING (T9.1).** |
| `elapsed_ms` | u64 | every timed span at close | Emitted by the `close` event of `info_span!` (subscriber config: `with_span_events(FmtSpan::CLOSE)`). **Status: PENDING — subscriber not configured with `FmtSpan::CLOSE` (T9.5).** |
| `source` | string | `log_frontend_error` ingest only | Always `"frontend"`. **Status: IMPLEMENTED.** |

### 1.3 Per-category schemas

All examples show the **fields** the code must attach via macro syntax. The wrapping JSON envelope (§1.1) is added by the subscriber. Each subsection ends with a **Current instrumentation status** note.

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

> **Current instrumentation status — PARTIAL.** `runs/session.rs` opens a `run_session` span
> (named `"run_session"`, not `"run"`; only `run_id`, not `project_id`/`sequence_name`) and emits
> `"run started"` (`run_id`, `project_id`, `pid`) and `"run finished"` (`run_id`, `status` via Debug,
> `exit_code` via Debug). **Missing:** `spawn_latency_ms`, `cli_path`, `attached_md`, `duration_ms`,
> `events_emitted`, `bytes_in`; a structured `spawn failed` error with `kind="cli"`; the `stop requested`
> reason taxonomy (cancel logs a generic info line, no `reason`). Covered by **T9.2**.

#### b) Event parsing (component=`event_parser`)

`info_span!("parse", run_id, mode)` where `mode` is `"heuristic"` (interactive mode only; stream-json removed in v1).

- **parse warning** — `tracing::warn!(kind = "parse_error", line_no, bytes_dropped, snippet, "malformed event")`
  - `line_no`: u64, line index within transcript
  - `bytes_dropped`: u32, length of malformed slice that became a `System` event
  - `snippet`: first 200 chars of the bad line
- **per-batch timing** (only if `elapsed_ms > 50`) — auto-emitted on span close via a dedicated `parse_batch` span: `info_span!("parse_batch", run_id, bytes_in)`.

> **Current instrumentation status — PARTIAL.** `runs/parser.rs` emits one `EventParser initialized`
> info line and correctly falls malformed/invalid-UTF8 lines back to `RunEvent::System` (so the transcript
> records them). **Missing:** the `parse_batch` span + `elapsed_ms`, and the structured `parse warning`
> (`kind="parse_error"`, `line_no`, `bytes_dropped`, `snippet`) — malformed lines are recorded as `System`
> events but never logged. Covered by **T9.2**.

#### c) Transcript writer (component=`transcript_writer`)

- **write failed** — `tracing::error!(kind = "io", correlation_id, run_id, file = "transcript.jsonl" | "raw.log" | "meta.json", io_kind, "transcript write failed")`
  - `io_kind`: `err.kind().to_string()` (e.g. `"PermissionDenied"`)
- **flush slow** — `tracing::warn!(run_id, elapsed_ms, file, "slow flush")` when a single flush takes > 100 ms
- **rotation note** (info, once per run start) — `tracing::info!(run_id, transcript_path, raw_log_path, "transcript opened")`

> **Current instrumentation status — PENDING.** `runs/transcript.rs` has **zero tracing**. Write/flush
> failures surface only via the `AppError`/`warn` lines in the caller (`session.rs`, e.g.
> `"append_events failed"`), which lack `component`, `kind`, `file`, and `io_kind`. Covered by **T9.2**.

#### d) Git poller (component=`git_poller`)

`info_span!("git_poll", project_id)` wraps `poll_one`.

- **poll ok** — auto-close emits `elapsed_ms`. Add explicit `tracing::debug!(branch, is_clean, dirty_files, ahead, behind, "git status")` inside the span at `debug` level (off by default).
- **poll slow** — `tracing::warn!(project_id, elapsed_ms, "slow git poll")` when `elapsed_ms > 1000`
- **poll error** — `tracing::warn!(kind = "git", project_id, git_error_class, message, "git poll failed")`
  - `git_error_class`: `"repo_missing" | "permission_denied" | "not_a_repo" | "other"`

> **Current instrumentation status — PARTIAL.** `projects/git.rs` logs the window focus/blur transition
> at `debug` (`component="git_poller"`) and a `warn` when a `spawn_blocking` git task panics. **Missing:**
> the `git_poll` span, `elapsed_ms`, the slow-poll warn, and the `git poll failed` warn with `kind="git"`
> and `git_error_class` classification. Today a failed poll is captured into `GitStatus.error` (a sanitized
> string from `git2::Error::message()`) and emitted on the `git:updated` event — the UI pill works, but
> there is no structured log line and no error-class taxonomy. Covered by **T9.3**.

#### e) IPC commands (component=`ipc`)

A single middleware-style wrapper macro `command_span!(name)` produces `info_span!("ipc", command = name, correlation_id)` for every `#[tauri::command]`. On span close, the subscriber emits `elapsed_ms`.

- **command error** — `tracing::error!(kind, correlation_id, command, message, "command failed")` inside the wrapper when the inner returns `Err(AppError)`.
- **command slow** — `tracing::warn!(command, elapsed_ms, "slow command")` when `elapsed_ms > 500` (but only for non-streaming commands; exclude `launch_run`, `load_transcript`).

> **Current instrumentation status — PENDING.** There is **no `instrument()` wrapper / `ipc` span** in
> `ipc/commands.rs`. Commands return `AppResult<T>` and Tauri serializes the `AppError`, but no central
> `"command failed"` error log is emitted, no per-command `correlation_id` is generated, and there is no
> slow-command warn. A handful of commands log ad-hoc info lines (`launch_run: run created`,
> `list_sequences called`, etc.). This is the keystone task — **T9.1** — that most other `correlation_id`
> work depends on.

#### f) Subprocess lifecycle (component=`run_manager` or `usage_probe`)

- **spawn** — `tracing::info!(component, cli_path, args = ?args, cwd = ?cwd, pid, "subprocess spawned")`
- **exit** — `tracing::info!(component, pid, exit_code, duration_ms, "subprocess exited")`
- **kill** — `tracing::info!(component, pid, signal = "SIGTERM" | "TerminateProcess", "subprocess killed")`

> **Current instrumentation status — PARTIAL.** The run subprocess `pid` is logged on `"run started"`
> and a kill is logged on cancellation (`"run cancellation requested — killing child"`), but without the
> `component`/`cli_path`/`args`/`cwd`/`signal`/`duration_ms` field set. The `usage_probe` subprocess does
> not exist yet (stub). Folded into **T9.2** (run side) / **T9.3** (usage side).

#### g) Usage probe (component=`usage_probe`)

`info_span!("usage_probe")` wraps `UsageProbe::fetch`.

- **fetch ok** — `tracing::info!(keys_parsed, elapsed_ms, "usage fetched")`
- **fetch failed** — `tracing::warn!(kind = "cli", exit_code, stderr_tail, "usage probe failed")`
- **parse failed** — `tracing::warn!(kind = "parse_error", raw_len, "usage parse failed")`
- **snapshot stale** — `tracing::warn!(age_secs, "usage snapshot stale")` (emitted when `get_usage` returns an entry older than `2 * usage_poll_interval_secs`)

> **Current instrumentation status — PENDING.** `usage/mod.rs` is a stub (`// UsageProbe goes here`);
> Epic 7 (usage/rate-limit pill) is not built. All §1.3g signals are pending. Covered by **T9.3**
> (which depends on T7.1).

#### h) Retention pruner (component=`retention_pruner`)

`info_span!("retention_run", trigger)` where `trigger` is `"startup" | "daily"`.

- **summary** — `tracing::info!(runs_pruned, bytes_freed, projects_scanned, elapsed_ms, "retention summary")`
- **per-deletion** — `tracing::info!(project_id, run_id, reason, age_days, size_bytes, "run pruned")`
  - `reason`: `"age" | "size"`
- **prune error** — `tracing::warn!(kind = "io", project_id, run_id, message, "retention delete failed")`

> **Current instrumentation status — IMPLEMENTED (mostly).** `runs/retention.rs` emits the per-deletion
> `"run pruned"` lines with `reason = "age" | "size"`, the run-level summary, and prune-error warns
> (`component="retention_pruner"`). **Minor gaps vs spec:** the prune-error warn omits `kind="io"`, and
> there is no named `retention_run` span carrying the `trigger` field (`startup`/`daily`). T9.3 reconciles
> the `kind` field; the `trigger` span is a minor nicety.

#### i) Orphan reaper (component=`orphan_reaper`)

`info_span!("orphan_reap")` wraps the startup scan.

- **scan summary** — `tracing::info!(candidates, killed, ignored, elapsed_ms, "orphan scan done")`
- **per-kill** — `tracing::info!(project_id, run_id, pid, exe_match = true, "orphan killed")`
- **ignored** — `tracing::info!(project_id, run_id, pid, exe_match = false, reason, "orphan ignored")`
  - `reason`: `"exe_mismatch" | "pid_dead" | "pid_not_found"`

> **Current instrumentation status — IMPLEMENTED (mostly).** `runs/orphan.rs` logs start
> (`"orphan reaper started"` with `project_count`, `cli_configured`), per-run discovery
> (`"found orphaned run"`), per-kill (`"sent kill signal …"` with `kill_sent`), skip reasons
> (exe mismatch / PID dead / CLI not configured), and a finish summary (`"orphan reaper finished"` with
> `killed`, `marked`). **Gaps vs spec:** message/field names differ (`"orphan reaper finished"` vs
> `"orphan scan done"`; `killed`/`marked` vs `candidates`/`killed`/`ignored`), no named `orphan_reap`
> span, no `elapsed_ms`. Functionally equivalent; low-priority reconciliation under T9.3.

#### j) Settings + CLI detection

- **settings load** — `tracing::info!(component = "settings_store", path, "settings loaded")` or `tracing::warn!(component = "settings_store", kind = "parse_error", path, archived_to, "settings corrupt, defaults restored")`
- **settings save** — `tracing::info!(component = "settings_store", elapsed_ms, "settings saved")` or `tracing::error!(component = "settings_store", kind = "io", message, "settings save failed")`
- **cli detect ok** — `tracing::info!(component = "cli_detect", resolved_path, version, mode = "interactive", "cli detected")`
- **cli detect failed** — `tracing::warn!(component = "cli_detect", kind = "cli", path_tried, message, "cli detect failed")`
- **cli lost** — `tracing::warn!(component = "cli_detect", last_known_path, "cli lost mid-session")`

> **Current instrumentation status — MIXED.**
> - **Settings (PARTIAL):** `settings/mod.rs` logs `"settings loaded"`, `"settings saved"`,
>   `"settings file not found; using defaults"`, and `"settings file is corrupt; archiving …"`.
>   Gaps: missing `component="settings_store"` on most lines, corrupt warn omits `kind="parse_error"`,
>   and the save-IO failure is not an `error!` with `kind="io"`. (T9.3)
> - **CLI detect ok/failed (IMPLEMENTED):** `verify_claude_cli` matches §2.13 exactly —
>   `component="cli_detect"`, `kind="cli"` on failure, `path_tried`, `message`, and the success line.
> - **cli lost (PENDING):** no `"cli lost mid-session"` warn found.

#### k) Frontend errors (component=`frontend`)

Forwarded via `log_frontend_error(message, stack, route?)`:

- `tracing::error!(component = "frontend", kind = "frontend", correlation_id, route, stack, "{message}")`

> **Current instrumentation status — PARTIAL.** The backend handler `log_frontend_error` in
> `ipc/commands.rs` matches the spec (`component="frontend"`, `source="frontend"`, `kind="frontend"`,
> generated `correlation_id`, sanitized `message`/`stack`/`route`). **Pending:** the React root error
> boundary that actually invokes it, and the toast CID chip. Covered by **T9.4**.

### 1.4 File and retention

- Path: `<os_config_dir>/dev-dashboard/logs/dev-dashboard.YYYY-MM-DD.log`
- Rotation: daily via `tracing_appender::rolling::daily`
- Retention: keep 7 files; on app startup, delete `dev-dashboard.*.log` older than 7 days
- Encoding: UTF-8, LF line endings (KB §6.4)
- Default level: `info`. Override via `DEV_DASHBOARD_LOG=debug|trace`. The env-var filter parses `tracing-subscriber::EnvFilter` directives, so `DEV_DASHBOARD_LOG="info,dev_dashboard::runs::parser=debug"` is supported and documented in Settings -> Open logs folder help text.

> **Current instrumentation status — PARTIAL.** `logging.rs` uses `tracing_appender::rolling::daily`
> with **file stem `app`** (so files are `app.YYYY-MM-DD.log`, not `dev-dashboard.YYYY-MM-DD.log`),
> a JSON file layer, a `warn`-only stderr layer, and `EnvFilter::try_from_env("DEV_DASHBOARD_LOG")`
> falling back to `info`. **Missing:** the startup retention sweep (delete files older than 7 days) and
> the Settings tooltip documenting the env-var syntax. Covered by **T9.5**.
>
> [adoption-assumption] The self-diagnosis recipes in §4 reference `dev-dashboard.<date>.log`; until the
> file stem is reconciled, the actual on-disk name is `app.<date>.log`. T9.5 should either rename the stem
> to `dev-dashboard` (preferred — matches the doc) or this doc should be updated to `app.<date>.log`.

---

## 2. Instrumentation Points (Rust, per component)

Format: file path -> function -> exact macro calls. All field names match §1. **Status tags reflect the 2026-06-13 audit.** Where the spec lists a module path that differs from the actual tree, the actual path is noted.

### 2.1 `src-tauri/src/lib.rs` (spec said `main.rs`) — startup — **PARTIAL**

- `logging::init_logging(&config_dir.join("logs"))` is called first in `lib.rs::run()`. It builds the `EnvFilter`, the daily appender, and the JSON layer.
- **Implemented:** `EnvFilter::try_from_env("DEV_DASHBOARD_LOG").unwrap_or_else(|_| EnvFilter::new("info"))`; daily rolling appender; `fmt::layer().json().with_current_span(true).with_span_list(true)`.
- **Missing (T9.5):** `.with_span_events(FmtSpan::CLOSE)` (no automatic `elapsed_ms`); the `app start` boot line (`version`, `os`, `log_dir`); the `app shutdown` line; the 7-day log-file retention sweep. File stem is `app` (spec wanted `dev-dashboard`).

### 2.2 `src-tauri/src/ipc/commands.rs` — IPC wrapper — **PENDING (T9.1)**

The `instrument<T, F, Fut>(name, f)` helper from the spec does not exist. To implement:

- Generate `correlation_id = Uuid::new_v4()`.
- Open `tracing::info_span!("ipc", command = name, correlation_id = %correlation_id)` and enter via `.instrument(span).await`.
- On `Err(e)`: `tracing::error!(kind = e.kind_str(), correlation_id = %correlation_id, command = name, message = %e, "command failed")` then attach `correlation_id` to `AppError::details`.
- Emit `warn!` if `elapsed_ms > 500` for non-streaming commands (exclude `launch_run`, `load_transcript`).
- **Blocker:** `AppError` needs a `kind_str()` returning the snake-case `kind` (today only `code()` → SCREAMING_SNAKE exists). `details` is currently hardcoded `null` in the manual `Serialize` impl — it must carry `{ "correlation_id": "…" }`.

### 2.3 `src-tauri/src/runs/session.rs` — `RunSession` (spec referenced `RunSession::start/on_exit/request_stop`) — **PARTIAL (T9.2)**

- Span opened at the `tokio::task::spawn` call site in `commands.rs::launch_run` as `info_span!("run_session", run_id)` (spec wanted `"run"` with `run_id`, `project_id`, `sequence_name`).
- `"run started"` present (`run_id`, `project_id`, `pid`); **missing** `spawn_latency_ms`, `cli_path`, `attached_md`.
- `"run finished"` present (`run_id`, `status` Debug, `exit_code` Debug); **missing** `duration_ms`, `events_emitted`, `bytes_in`; status/exit should be display-formatted strings.
- **Missing** the structured `spawn failed` error (`kind="cli"`, `stderr_tail`) and the `stop requested` reason taxonomy.

### 2.4 `src-tauri/src/runs/parser.rs` — `EventParser` — **PARTIAL (T9.2)**

- `EventParser::new()` logs `pattern_version` + `"EventParser initialized"`.
- **Missing:** `parse_batch` span + `elapsed_ms`; the `parse warning` (`kind="parse_error"`, `component="event_parser"`, `line_no`, `bytes_dropped`, `snippet`). Malformed/invalid-UTF8 lines already become `RunEvent::System` (transcript record kept) — only the structured warn is missing.

### 2.5 `src-tauri/src/runs/transcript.rs` — `TranscriptWriter` — **PENDING (T9.2)**

- Zero tracing. Need `transcript opened` info, `transcript write failed` error (`kind="io"`, `file`, `io_kind`), and the slow-flush warn (> 100 ms).

### 2.6 `src-tauri/src/projects/git.rs` — git poll loop — **PARTIAL (T9.3)**

- Window focus/blur `debug!` and a panic `warn` exist. Need the `git_poll` span, `elapsed_ms`, slow-poll warn (> 1000 ms), and `git poll failed` warn with `kind="git"` + `git_error_class` (classify `git2::Error::code()`/`class()`). Note: the synchronous git work is `git_status_for_path` run via `spawn_blocking`, so timing/classification must wrap the blocking call.

### 2.7 `src-tauri/src/usage/mod.rs` — `UsageProbe` — **PENDING (T9.3 / T7.1)**

- Module is a stub. All §1.3g signals pending until Epic 7 builds the probe.

### 2.8 `src-tauri/src/runs/retention.rs` — `RetentionPruner` — **IMPLEMENTED (mostly) (T9.3)**

- `component="retention_pruner"`, per-deletion `"run pruned"` (`reason = age|size`), summary, and prune-error warns present. Add `kind="io"` to the prune-error warn; optional `retention_run` span with `trigger`.

### 2.9 `src-tauri/src/runs/orphan.rs` — orphan reaper — **IMPLEMENTED (mostly) (T9.3)**

- `component="orphan_reaper"` start/found/kill/skip/finish lines present. Reconcile message/field names to the spec (`orphan scan done`, `candidates`/`killed`/`ignored`, per-kill `exe_match`) if exact-match acceptance is required; otherwise functionally complete.

### 2.10 `src-tauri/src/sequences/mod.rs` — `SequenceLoader` — **IMPLEMENTED (mostly) (T9.3)**

- `component="sequence_loader"`, `SequenceLoader::load_all` span, and warns present. Add `kind="parse_error"` to the non-utf8 fallback warn.

### 2.11 `src-tauri/src/projects/mod.rs` — `ProjectRegistry` — **IMPLEMENTED (mostly) (T9.3)**

- `component="project_registry"` load/save + corrupt/IO warns present. Add `kind` fields, promote the IO-error path to `error!`, and add named `registry_load`/`registry_save` spans with `elapsed_ms` if exact-match acceptance is required.

### 2.12 `src-tauri/src/settings/mod.rs` — `SettingsStore` — **IMPLEMENTED (mostly) (T9.3)**

- `"settings loaded"` / `"settings saved"` / corrupt-archive warns present. Add `component="settings_store"` consistently, `kind="parse_error"` on corrupt, and an `error!` with `kind="io"` on save failure.

### 2.13 `src-tauri/src/ipc/commands.rs` — `verify_claude_cli` — **IMPLEMENTED**

- Matches the spec: success `"cli detected"` (`component="cli_detect"`, `resolved_path`, `version`, `mode="interactive"`); failure `"cli detect failed"` (`kind="cli"`, `path_tried`, `message`).

### 2.14 `src-tauri/src/ipc/commands.rs` — `log_frontend_error` — **IMPLEMENTED (backend) / PENDING (FE) (T9.4)**

- Backend matches §1.3k. The React root error boundary that calls it is pending.

### 2.15 Window focus bridge — **IMPLEMENTED (debug)**

- `projects/git.rs` logs `component="git_poller", state="focused"|"blurred"` at `debug` on window focus change — matches the §2.15 intent (spec used `component="window_focus"`; actual uses `git_poller`).

---

## 3. In-App Health Signals (UI surfaces)

Every signal has: (a) the user-visible UI element, (b) the source log event(s), (c) the trigger condition.

| UI surface | What it shows | Source signal | Trigger | Status |
|---|---|---|---|---|
| **Rate-limit pill** (top bar, UI §5.2) | KV pairs from `claude /usage`, or `--` placeholder | `usage:updated` event (data) + `usage_probe` warn logs | `available=false` -> render `--`; popover shows "Last check failed" with the `correlation_id` | **PENDING** — usage module is a stub (Epic 7). |
| **CLI-lost banner** (top of dashboard) | "Claude CLI not found at <path>. Open Settings." | `cli:lost` event | 60s detect loop transition `found -> not-found` | **PARTIAL** — `cli_watcher` exists; verify the `cli:lost` emit + the `cli lost` log line. |
| **Project card git error pill** | Red dot + tooltip "Git status unavailable: <class>" | `git:updated` event with `status.error = Some(...)` | Last poll returned an error for that project | **IMPLEMENTED (data)** — `GitStatus.error` is populated + emitted; the structured `git poll failed` warn behind it is PENDING (T9.3). |
| **Project card missing state** (UI §5.2) | Greyed card, "Folder missing — Relocate?" | `project:missing` event | `path.exists() == false` | **IMPLEMENTED** — `is_missing` computed live in `list_projects`. |
| **Run failure toast** (UI §5.9) | "Sequence X failed — open log" with `correlation_id` chip | `toast:show` on `run:finished` with `status=failed` | `run:finished` with failure | **PARTIAL** — `run:finished` carries `status`; the `correlation_id` chip is PENDING (T9.4). |
| **Transcript-unavailable state** (S-05) | Error card with [Open folder] | `load_transcript` AppError | `load_transcript` returns `ParseError`/`Io` | **DEPENDS** — `load_transcript` is Epic 5 territory; verify per that epic. |
| **Step-failure prompt card** (UI §5.4) | Inline Retry/Skip/Abort/Continue card | `run:step_failure` event | Parser emits `StepFailed` | **PARTIAL** — `RunEvent::StepFailed` exists but the parser sentinel is currently empty (`STEP_FAILED_SENTINEL == ""`), so it never fires yet (current branch T4.7 work). |
| **Settings -> "Open logs folder" button** | Reveals log dir in OS file manager | n/a | Always available | **IMPLEMENTED** — `open_logs_folder` command present. |
| **Frontend error toast** (dev mode only) | Generic "Internal error (CID: …)" | `log_frontend_error` -> `tracing::error!(kind=frontend)` | React error boundary catches unhandled render error | **PARTIAL** — backend pipe ready; error boundary PENDING (T9.4). |

Rate-limiting in the UI: at most one CLI-lost banner; toasts already bounded to 4 visible (UI §5.9); a project that emits 10 git errors in a minute only shows one pill (not a flood).

---

## 4. Self-Diagnosis Guide

All recipes assume the logs folder is open (Settings -> Open logs folder). Suggested tools: `jq`, plain editor search. JSON-per-line means each log is a single object on one line.

> [adoption-assumption] Until T9.5 reconciles the file stem, the on-disk file is `app.<date>.log`,
> not `dev-dashboard.<date>.log`. Substitute the actual stem in the commands below.

### 4.1 Run fails immediately

1. Find the run id (toast body, or `meta.json`).
2. `jq -c 'select(.run_id == "<id>")' app.<date>.log`
3. Look for:
   - `"spawn failed"` with `kind=cli` and `stderr_tail` -> CLI rejected the args or the binary path is wrong. *(pending T9.2 — until then check the `AppError::Io` from `launch_run` spawn.)*
   - `"run started"` followed by `"run finished"` with `status=failed` and tiny `duration_ms` -> child exited; check `exit_code` and `transcript.jsonl`.
   - No matching entries at all -> search for the `correlation_id` shown in the toast *(pending T9.1)*.

### 4.2 Git status stops updating

1. `jq -c 'select(.component == "git_poller")' app.<date>.log | tail -n 50`
2. Look for:
   - Repeated `"git poll failed"` with `git_error_class=repo_missing` -> project moved *(pending T9.3; today inspect the `git:updated` event payload's `status.error` instead)*.
   - `"slow git poll"` repeating -> repo is huge or on a network share; bump `git_poll_interval_secs`.
   - **No log lines at all in the last minute** -> poller paused; check the `state="blurred"` debug line (re-run with `DEV_DASHBOARD_LOG=debug`).

### 4.3 Usage bar shows `--`

*(Pending Epic 7 — the usage probe is not built; no `usage_probe` logs exist yet.)*

### 4.4 App startup slow

1. `jq -c 'select(.fields.message == "orphan reaper finished" or .fields.message == "retention summary" or .fields.message == "settings loaded")' app.<date>.log | head -n 20`
2. The orphan/retention/registry/settings phases each log a line; compare timestamps. *(per-phase `elapsed_ms` pending T9.5/T9.3.)*

### 4.5 Transcript missing in history view

1. From the affected run, note `run_id`.
2. `jq -c 'select(.run_id == "<id>")' app.<date>.log`
3. Look for `append_events failed` / `transcript writer close failed` warns from `session.rs` *(the structured `transcript write failed` with `kind=io`/`io_kind` is pending T9.2)*, or `"run pruned"` for the same `run_id` (retention deleted it).

### 4.6 Generic recipes

- **Errors by kind (top 5)**: `jq -c 'select(.level == "ERROR") | .fields.kind' app.*.log | sort | uniq -c | sort -rn | head -n 5` *(meaningful once T9.1–T9.3 add `kind` broadly)*
- **One run, full trace**: `jq -c 'select(.run_id == "<id>")' app.*.log`
- **Follow a correlation_id**: `jq -c 'select(.correlation_id == "<cid>")' app.*.log` *(today only frontend errors carry one)*

---

## 5. External tool config

None. This is a local-only app (NFR-8): no external monitoring service, no alerting, no dashboards, no outbound network. The "dashboard" is the developer's `jq`/editor over the on-disk JSON logs, and the in-app health pills (§3).

---

## 6. Setup / Gap Tasks → Epic 9

The original spec's setup tasks map 1:1 to **Epic 9 — Observability** (`docs/epics/epic-9-observability/`), which already exists. No new tasks are emitted by this adoption pass — the pending instrumentation surfaced in the audit is fully covered.

| Task | Covers | Audit status it closes |
|---|---|---|
| **T9.1** IPC instrumentation middleware | §1.3.e, §2.2; `AppError::kind_str()` + `correlation_id` in `details` | IPC wrapper PENDING; `kind`/`correlation_id` plumbing PARTIAL |
| **T9.2** Run + transcript + parser logging | §2.3, §2.4, §2.5 | run_session PARTIAL, parser PARTIAL, transcript PENDING |
| **T9.3** Background-task logging (git, usage, retention, orphan, sequence, registry, settings) | §2.6–§2.12 | git PARTIAL, usage PENDING, retention/orphan/sequence/registry/settings IMPLEMENTED-mostly (reconcile `kind`/spans) |
| **T9.4** Frontend error pipe + correlation_id surfacing | §1.3.k, toast CID chip | frontend pipe PARTIAL (FE boundary pending) |
| **T9.5** EnvFilter + log retention sweep + Settings tooltip | §1.4, §2.1 | EnvFilter IMPLEMENTED; `FmtSpan::CLOSE`/`elapsed_ms`, retention sweep, file-stem reconciliation, tooltip PENDING |

> [adoption-assumption] T9.3's acceptance criteria reference `usage.rs`, `sequences/mod.rs`, etc.; the
> actual module paths are `usage/mod.rs`, `sequences/mod.rs`, `settings/mod.rs`, `projects/mod.rs`. The
> Coder should target those. T9.3 also depends on T7.1 (usage) which is unbuilt — the usage portion of
> T9.3 cannot complete until Epic 7 lands.
>
> One reconciliation item worth flagging to the Coder picking up T9.5: the log file stem is `app`, not
> `dev-dashboard`; either rename it (preferred, matches this doc and the self-diagnosis recipes) or update
> the docs. This is in-scope for T9.5's "log retention sweep" work since it touches `init_logging`.
