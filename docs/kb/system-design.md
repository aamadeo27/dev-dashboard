# System Design

## 1. System Design

### 1.1 Process model

The application is a single Tauri window with a Rust core. The Rust side owns all stateful, long-lived, or filesystem-touching work; the web frontend is a thin rendering layer.

```
+---------------------------------------------------------------+
|                       Tauri Application                       |
|                                                               |
|  +---------------------------+   +-------------------------+  |
|  |   WebView (Frontend)      |   |     Rust Core           |  |
|  |   - React + TS            |<->|   (Tauri commands +     |  |
|  |   - Zustand stores        |   |    background tasks)    |  |
|  |   - View components       |   |                         |  |
|  +---------------------------+   |   - ProjectRegistry     |  |
|             ^   |                |   - GitPoller           |  |
|             |   | IPC            |   - SequenceLoader      |  |
|             |   v (commands)     |   - RunManager          |  |
|             |                    |   - TranscriptWriter    |  |
|             |   events (push)    |   - UsageProbe          |  |
|             +-------------------+|   - SettingsStore       |  |
|                                  |   - RetentionPruner     |  |
|                                  +-------------------------+  |
|                                            |                  |
|                                            v                  |
|              +-----------------------------------------+      |
|              |  Child processes (one per run)          |      |
|              |   - claude CLI (per-project cwd)        |      |
|              |   - claude /usage (transient, on poll)  |      |
|              +-----------------------------------------+      |
|                                            |                  |
|                                            v                  |
|              +-----------------------------------------+      |
|              |  Filesystem                             |      |
|              |   - OS config dir: settings, registry   |      |
|              |   - <project>/.claude/runs/<id>/...     |      |
|              |   - <project>/.claude/sequences/*.md    |      |
|              +-----------------------------------------+      |
+---------------------------------------------------------------+
```

### 1.2 Components and responsibilities (Rust core)

| Component | Responsibility |
|---|---|
| `ProjectRegistry` | Owns the registered project list (CRUD). Persists to `projects.json` in OS config dir. Detects missing directories on read. |
| `ProjectScanner` | On registration / refresh, scans a directory for `.claude/`, detects language/PM (heuristics), reads `last_modified`. |
| `SequenceLoader` | Reads `<project>/.claude/sequences/*.md`; extracts name (filename minus `.md`) and description (first non-heading paragraph). Caches per project, invalidated on FS mtime change. |
| `GitPoller` | Per-project git status polling. Single tokio task per visible project; pauses when window unfocused/hidden. Uses `git2` crate (libgit2 bindings) — no shell out. |
| `RunManager` | Spawns Claude CLI child processes, owns one `RunSession` per active run. Routes stdin in, multiplexes stdout/stderr out. Tracks state. Unlimited concurrency. |
| `RunSession` | Per-run state machine: `pending -> running -> {completed, failed, stopped}`. Owns the child handle, the parser, the transcript writer. |
| `EventParser` | Heuristic streaming parser that reads interactive Claude stdout line-by-line, using pattern matching to detect event boundaries (tool calls, thinking blocks, file edits, assistant text). Stateful; buffers partial output. Documents exact patterns in §6.7. |
| `TranscriptWriter` | Append-only JSONL writer for `transcript.jsonl`, raw passthrough writer for `raw.log`, and `meta.json` updater. Flushes on each event (durability over throughput). |
| `UsageProbe` | Runs `claude /usage`, parses key-value stdout, exposes latest snapshot. Scheduled every 60s + on-demand. |
| `SettingsStore` | Loads/saves `settings.json` in OS config dir. Single source of truth for all configurable values. |
| `RetentionPruner` | On startup and once daily (24h timer), walks `<project>/.claude/runs/`, prunes runs exceeding age or size thresholds. Lowest-mtime-first. |
| `OrphanReaper` | On startup, reads any `meta.json` files with status `running` or `pending`, sends SIGTERM/TerminateProcess to their PID (if alive and matches expected exe name), marks runs as `failed` with note "Terminated (app restarted)". |
| `WindowFocusBridge` | Subscribes to Tauri window focus/blur/visibility events; broadcasts to `GitPoller` and `UsageProbe` to pause/resume. |

### 1.3 Data flow: launching a run

```
Frontend                Rust Core                       Child
   |                       |                              |
   |--launch_run(...)----->|                              |
   |                       |--spawn claude (cwd=proj)---->|
   |                       |  write meta.json (pending)   |
   |<--Ok(run_id)----------|                              |
   |                       |--state=running, emit event-->|
   |<--evt:run_started-----|                              |
   |                       |<--stdout bytes---------------|
   |                       |  parse to RunEvent           |
   |                       |  append transcript.jsonl     |
   |                       |  append raw.log              |
   |<--evt:run_event-------|                              |
   |  (per parsed event)   |                              |
   |                       |                              |
   |--send_input(run_id,   |                              |
   |    "hello\n")-------->|--write to child stdin------->|
   |<--Ok------------------|                              |
   |                       |<--exit code------------------|
   |                       |  state=completed/failed      |
   |                       |  update meta.json            |
   |<--evt:run_finished----|                              |
```

### 1.4 Data flow: dashboard render

```
App start
  -> SettingsStore.load()
  -> ProjectRegistry.load()
  -> OrphanReaper.run()       // before any new run
  -> RetentionPruner.run()    // once at startup
  -> emit projects:loaded
Frontend mounts:
  -> invoke list_projects() -> renders cards
  -> per project: invoke get_git_status(id) -> updates card
  -> GitPoller subscribes to focused project ids (visible cards) -> pushes git:updated events on interval
```

---

## Other

### 7. Monitoring

This section sets direction only — the Monitor agent owns concrete queries, dashboards, and alert rules.

#### 7.1 Level

**Basic**, locally-scoped. No remote telemetry (NFR-8). All monitoring data lives on the user's machine and is for **the user's own debugging**. No outbound sinks.

#### 7.2 Tool family

- **Logging**: `tracing` + `tracing-appender` writing to `<os_config_dir>/dev-dashboard/logs/dev-dashboard.YYYY-MM-DD.log` (daily rotation, keep 7 days).
- **In-app log viewer**: deferred. v1: the user opens the file in their editor.
- **Frontend errors**: caught by a root error boundary -> logged via `console.error` and forwarded to Rust via a `log:frontend_error` command for inclusion in the rotated log file.

Log format: structured JSON per line (`tracing_subscriber::fmt::Layer::json()`), default level `info` for production, `debug` when `DEV_DASHBOARD_LOG=debug` env var is set.

#### 7.3 Must-have signals

| Signal | Where | Why |
|---|---|---|
| **Errors by category** | `tracing::error!` with `kind=` field | First thing the user grep's after a crash |
| **Run start/stop/duration** | `info` span around `RunSession::run()` | Did the run actually start? How long did it take? |
| **Parser failures** | `warn` with offending line snippet | The single most fragile part of the system |
| **CLI spawn failures** | `error` with exit code + stderr tail | "Why didn't my run start?" |
| **Git poll latency** | timed span per poll | Diagnose slow repos |
| **Usage probe failures** | `warn` with stderr | "Why does my rate-limit pill say --?" |
| **Retention pruner activity** | `info` per pruned run with reason (age/size) | "Where did my old runs go?" |
| **Orphan reaper activity** | `info` per killed PID | "Why is this run marked failed?" |
| **IPC command latency** | timed span per command | Diagnose UI hangs |
| **FS write errors on transcript** | `error` — must never silently lose events | Data integrity |

#### 7.4 Operations needing timing instrumentation

Wrap in a `tracing::info_span!` with elapsed time logged on close:
- `RunManager::launch_run` (spawn -> first event)
- `EventParser::feed` (per-batch, only log if > 50 ms)
- `GitPoller::poll_one`
- `UsageProbe::fetch`
- `SequenceLoader::load_all`
- `RetentionPruner::run`
- `ProjectRegistry::load` and `save`

#### 7.5 Error categories to track

`AppError` variants double as error taxonomy. Each `error!` log includes `kind = <variant_name>`. Monitor agent: counts by `kind` over time, surface the top three weekly.

#### 7.6 Self-diagnosis affordances

- **About / Diagnostics screen** (deferred to v1.1, but plan for it): app version, CLI version, log file path button "Reveal in Finder/Explorer", config dir path, OS info, list of running PIDs.
- For v1, the user reaches the log file via Settings -> "Open logs folder" button (low-cost addition; include it).
- Every `error` log line includes a `correlation_id` that — when relevant — is also surfaced in the failing toast's body so the user can grep for it.

---

### 8. Out-of-Scope Confirmations (carry-over)

These are enumerated in Requirements section 6 and remain out-of-scope at the architecture level:
- Sequence authoring/editing in-app
- Pause/resume runs
- Concurrency limits / queueing
- OS-native notifications
- Cloud sync, remote access
- Cross-project run history view

Architecture choices intentionally do not preclude future work in these areas (e.g., the event stream design is compatible with pause/resume; the run storage layout is compatible with global aggregation), but no scaffolding for them ships in v1.

---

### 9. Open Items for the Coder

Items where the architecture defers to implementation discovery. Coder must validate and report back; document the resolution.

1. **EventParser heuristic patterns**: must be validated against actual interactive Claude CLI output before T4.1 is complete. Capture a representative session transcript, list each pattern in §6.7, and confirm a real-world example matches. Record the CLI version tested in `runs/parser/patterns.rs` as the pattern-set baseline.
2. **`claude /usage` stdout format**: verify the exact parseable shape; the data model assumes a `BTreeMap<String, String>` which is forgiving.
3. **Step-failure interaction protocol** (BLOCKER-02, T4.8): **RESOLVED — option (b) kill + re-invoke** (conservative default; see T4.8 task doc for full reasoning).

   **Evidence reviewed (T4.8)**:
   - T1.2 probe only ran `claude --version` with `.stdin(Stdio::null())`; no step-failure interactive behavior was observed or captured.
   - The EventParser (`patterns.rs`) has `STEP_FAILED_SENTINEL = ""` (disabled placeholder); no "waiting for input" marker was ever identified in CLI output.
   - `RunSession` has a generic `input_rx` stdin channel but no step-failure-specific state or routing.
   - No Claude CLI documentation, `--help` capture, or prior transcript shows a stdin-token prompt for step recovery.

   **Resolution — (b) kill + re-invoke**, per-choice plan:

   | Choice | Implementation |
   |---|---|
   | **Continue** | No kill needed. Write `"\n"` (empty line) to stdin as a "proceed" signal. If that does not unblock the CLI within a 2 s window, fall back to kill + re-invoke the original prompt with `--continue` flag if available, or bare re-invoke. Auto-triggered after 60 s timeout. |
   | **Retry** | Kill the current child process (`child.kill()`). Re-invoke Claude CLI with the same `LaunchInput` (same prompt and attached context). A new `run_id` is minted; the UI links it to the original as a retry. |
   | **Skip** | Kill the current child process. Re-invoke Claude CLI with a modified prompt that instructs the model to skip the failing step and continue from the next one (prompt prefix: `"Skip the previous failing step and continue."` followed by the original prompt). |
   | **Abort** | Kill the current child process (`child.kill()`). Mark the run `RunStatus::Failed` with `exit_note = "Aborted by user"`. Do not re-invoke. |

   **Rationale for conservative default**: option (b) is correct regardless of whether a stdin token protocol exists. If a future Claude CLI version adds stdin tokens (e.g. pressing `y`/`n` at a recovery prompt), option (b) can be enhanced to try the token first and fall back to kill+re-invoke on timeout. Implementing (b) first gives a working system with no risk of blocking on an unconfirmed stdin protocol.
4. **PID matching on orphan reap**: confirm the process name reported by `sysinfo` matches `claude` (or the configured CLI path) across all three OSes. Conservative: only kill if PID is alive AND exe path matches the configured CLI path.
