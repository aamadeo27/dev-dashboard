# Epics and Tasks: Dev Dashboard v1

**Date**: 2026-05-18
**Companion doc**: `knowledge-base.md` (read first)

## Conventions

- Each task is tagged `[frontend]`, `[backend]`, or `[shared]`.
- Each task has: **Description**, **Acceptance Criteria**, **Dependencies** (task IDs).
- Tasks are sized for a single Sonnet coding session (~2–4h).
- Order is dependency-respecting; within a level, `[frontend]` and `[backend]` may run in parallel.
- "KB §X.Y" references the Knowledge Base section.

---

## Epic 0 — Project Bootstrap and Plumbing

Foundation. Blocks everything.

### T0.1 [shared] Scaffold Tauri 2 + React + TS project

- **Description**: `pnpm create tauri-app` with React + TS + Vite template, Tauri v2. Rename to `dev-dashboard`. Configure Biome, rustfmt, clippy. Add the file layout from KB §4 as empty modules/files (no logic yet).
- **Acceptance**:
  - `pnpm tauri dev` launches an empty window on the dev machine.
  - `pnpm lint` and `cargo clippy -- -D warnings` both pass on the scaffold.
  - Directory tree matches KB §4 (empty files/`pub mod` declarations are fine).
- **Dependencies**: none.

### T0.2 [backend] AppState, AppError, command registration skeleton

- **Description**: Implement `AppState` (held in `tauri::State`), `AppError` enum + `AppResult<T>`, error serialization to `{ code, message, details? }`. Register a single `ping() -> String` command end-to-end as a smoke test.
- **Acceptance**:
  - `AppError` variants per KB §6.2; `From<std::io::Error>` and `From<git2::Error>` impls.
  - Frontend can `invoke('ping')` and get back `"pong"`.
  - Throwing an `AppError::NotFound` in a command results in a structured JS error object on the frontend.
  - If `DEV_DASHBOARD_CONFIG_DIR` env var is set, use it as the config directory instead of the OS default. Log the resolved path at INFO level on startup.
- **Dependencies**: T0.1.

### T0.3 [shared] ts-rs binding generation

- **Description**: Wire `ts-rs` so all DTOs in KB §3 export to `src/ipc/bindings.ts`. Add a `pnpm bindings` script that runs `cargo test --features export-bindings`. Add a CI/pre-commit check that the generated file is up to date.
- **Acceptance**:
  - Adding a `#[derive(TS)]` to a struct regenerates the TS file.
  - `Project`, `Sequence`, `Run`, `RunEvent`, `Settings`, `UsageSnapshot`, `GitStatus`, `LaunchInput`, `StepFailureChoice`, `CliCheck` all exported.
- **Dependencies**: T0.2.

### T0.4 [shared] IPC wrappers and event bus

- **Description**: Build `src/ipc/commands.ts` with typed wrappers for every command in KB §5 (stubbed against not-yet-implemented Rust commands is fine — types only). Build `src/ipc/events.ts` with a typed `subscribe(eventName, handler)` helper. Centralize event-name constants in `src/ipc/events.ts` and `src-tauri/src/ipc/events.rs`.
- **Acceptance**:
  - No string event names appear outside `events.ts` / `events.rs`.
  - All command wrappers compile against `bindings.ts` types.
- **Dependencies**: T0.3.

### T0.5 [frontend] Design tokens, global styles, router shell

- **Description**: Translate UI spec §1 into `src/styles/tokens.css` (all CSS variables). Set up `react-router-dom` with placeholder routes for S-01 through S-07. App shell renders the appropriate empty placeholder for each route.
- **Acceptance**:
  - Navigating between routes works; placeholders show the screen ID.
  - Token vars are accessible from any component.
  - Reduced-motion media query disables transitions globally (UI §1.6).
- **Dependencies**: T0.1.

### T0.6 [backend] Logging and tracing setup

- **Description**: Initialize `tracing` + `tracing-subscriber` with JSON formatter and daily-rotating file appender at `<os_config_dir>/dev-dashboard/logs/`. Read log level from `DEV_DASHBOARD_LOG` env var. Add a `log:frontend_error` command that ingests frontend errors.
- **Acceptance**:
  - Log file appears in the OS config dir on launch.
  - `tracing::info!("hello")` from main shows up structured in the log.
  - Calling `invoke('log_frontend_error', { message, stack })` writes a line at `error` level with `source=frontend`.
- **Dependencies**: T0.2.

---

## Epic 1 — Settings and CLI Detection

Unblocks S-01 (Setup) and S-07 (Settings).

### T1.1 [backend] SettingsStore

- **Description**: Implement load/save for `settings.json` in OS config dir. Validate ranges (KB §3.5). Provide a `SettingsPatch` for partial updates. Atomic write (tmp file + rename).
- **Acceptance**:
  - First launch creates a default settings file.
  - `get_settings` and `update_settings` commands work.
  - Corrupted settings file -> falls back to defaults, logs a warning, archives the broken file with `.broken` suffix.
- **Dependencies**: T0.2, T0.6.

### T1.2 [backend] verify_claude_cli command

- **Description**: Resolve the CLI path (override > PATH lookup), spawn `<path> --version`, parse output, return `CliCheck`. Interactive-mode invocation (no `--print`, no `--output-format`) is the v1 contract — no additional flag probing required.
- **Acceptance**:
  - Returns `found=true` with version string when CLI is installed.
  - Returns `found=false` with a helpful error when missing.
  - Logs the resolved path at `info` level.
- **Dependencies**: T1.1.

### T1.3 [frontend] S-01 Setup screen

- **Description**: Implement UI spec §5.1. On app load, call `verify_claude_cli(undefined)`; if not found, route to `/setup`. The setup screen shows OS-detected install instructions, a path input with Browse, and a Verify button calling `verify_claude_cli(path)`.
- **Acceptance**:
  - All states from UI §5.1 (Initial, Verifying, Success, Failure) render correctly.
  - Successful verification persists the path via `update_settings({ claude_cli_path })` and routes to `/`.
  - Browse button opens an OS file picker filtered to executables.
- **Dependencies**: T1.2, T0.5.

### T1.4 [frontend] S-07 Settings screen

- **Description**: Implement UI spec §5.7. Form-bound to `Settings`. Save calls `update_settings`. Includes "Open logs folder" button (calls a new `open_logs_folder` command). Unsaved-changes prompt on back.
- **Acceptance**:
  - All fields from UI §5.7 are present with described validation.
  - Saved confirmation appears for 2s after successful save.
  - View toggle here stays in sync with Dashboard toolbar (shared via Zustand `ui` store).
- **Dependencies**: T1.1, T0.5.

### T1.5 [backend] open_logs_folder command

- **Description**: Use `tauri-plugin-shell` to reveal the log folder in OS file manager.
- **Acceptance**: Works on Win/macOS/Linux. Errors surface as `AppError::Io`.
- **Dependencies**: T0.6.

### T1.6 [backend] cli:lost detection

- **Description**: Background task that re-runs `verify_claude_cli` on a 60s interval (paused when window unfocused). On transition from found -> not-found, emit `cli:lost` event.
- **Acceptance**:
  - Renaming the CLI binary mid-session emits the event within ~60s.
  - Restoring it stops further `cli:lost` emissions on next check.
- **Dependencies**: T1.2.

---

## Epic 2 — Project Registry and Git Status

Unblocks S-02 (Dashboard).

### T2.1 [backend] ProjectRegistry CRUD

- **Description**: Implement `list_projects`, `add_project`, `remove_project`, `relocate_project`, `set_project_tags`, `rename_project`. Persist to `projects.json` atomically. Canonicalize paths. Reject duplicate paths.
- **Acceptance**:
  - Adding the same path twice returns `AppError::AlreadyExists`.
  - Tags are lowercased, trimmed, deduped before save.
  - `list_projects` populates `is_missing` by `path.exists()` check.
- **Dependencies**: T0.2.

### T2.2 [backend] ProjectScanner (language/PM detection)

- **Description**: On `add_project` and on demand, detect language and package manager via marker files: `Cargo.toml` -> rust/cargo; `package.json` + `pnpm-lock.yaml` -> ts/pnpm; etc. Update fields on the `Project`.
- **Acceptance**:
  - At least 6 stacks detected: rust/cargo, ts/pnpm, ts/npm, python/uv, python/poetry, go/gomod.
  - Unknown projects: `language=None, package_manager=None` — no error.
- **Dependencies**: T2.1.

### T2.3 [backend] GitPoller

- **Description**: Per-project git status via `git2`. Returns `GitStatus`. A central poller task tracks "visible project ids" set; polls each visible project at `git_poll_interval_secs`. Pauses on window blur. Emits `git:updated` events.
- **Acceptance**:
  - `get_git_status(id)` returns clean/dirty/ahead/behind correctly on a test repo.
  - Visible-set updates via a `set_visible_projects(ids: Vec<String>)` command from the frontend.
  - Polling pauses within 1s of window blur and resumes on focus.
- **Dependencies**: T2.1, T1.1.

### T2.4 [frontend] useProjects + useGitStatus hooks

- **Description**: TanStack Query hook backed by `list_projects`. `useGitStatus(id)` reads from a Zustand store kept in sync by `git:updated` events. `useVisibleProjects` reports visible card ids to backend via `set_visible_projects` (debounced).
- **Acceptance**:
  - Mounting the Dashboard registers visible project ids within 500ms.
  - Hiding the window (window:blur) reports an empty visible set.
- **Dependencies**: T2.3, T0.4.

### T2.5 [frontend] ProjectCard component

- **Description**: Implement UI §5.2 grid card. Includes status edge color, name, tag chips, git badge, last-run badge, quick-run button. Missing-state variant. Loading skeleton variant.
- **Acceptance**:
  - All five card states from UI §5.2 render against mock data.
  - Quick-run button shows correct tooltip per prior-run state.
- **Dependencies**: T0.5.

### T2.6 [frontend] S-02 Dashboard layout, toolbar, empty state

- **Description**: Top bar (logo, rate-limit pill placeholder, gear), toolbar (Add Project, search, tag filter chips, view toggle), grid/list switcher, empty state. Wire to `useProjects`.
- **Acceptance**:
  - Add Project opens a directory picker via `tauri-plugin-dialog` and calls `add_project`.
  - Search filters cards by name or path on each keystroke.
  - Tag filter chips reflect the union of all project tags.
  - View toggle persists via Settings.
- **Dependencies**: T2.5, T2.4, T1.4.

### T2.7 [frontend] Project card context menu + tag editor popover

- **Description**: Right-click context menu (Open in Editor, Open in Terminal, Edit Tags, Remove, Relocate-if-missing). Tag editor popover (S-08) anchored to card.
- **Acceptance**:
  - All five context items work.
  - Tag changes update the card in real time.
  - Remove shows inline confirm; cancel restores card.
- **Dependencies**: T2.6, T2.8.

### T2.8 [backend] open_in_editor / open_in_terminal commands

- **Description**: Implement per KB §6.7 / UI §5.2 / GAP-08: `$EDITOR` env var first; OS default file association fallback. Default terminal launch per OS.
- **Acceptance**:
  - Works on all three OSes (manual smoke).
  - Failure emits an error toast via `toast:show` and returns `AppError::Io`.
- **Dependencies**: T0.2.

---

## Epic 3 — Sequences

Unblocks Project Detail sequences panel and Launch Modal.

### T3.1 [backend] SequenceLoader

- **Description**: `list_sequences(project_id)`, `refresh_sequences(project_id)`. Reads `<project>/.claude/sequences/*.md`. Extracts description (first non-heading paragraph; `(No description)` fallback). Caches in-memory keyed by project_id; invalidates on directory mtime change.
- **Acceptance**:
  - Empty dir -> empty Vec, no error.
  - Description extraction handles: heading-only files, blank files, multi-paragraph files, Windows line endings.
  - Cache invalidates within one call after mtime change.
- **Dependencies**: T2.1.

### T3.2 [frontend] useSequences hook + SequenceRow component

- **Description**: Hook wraps `list_sequences`. Component renders name, description, [Run] button. Selected + hover states.
- **Acceptance**:
  - Empty state matches UI §5.3 / §5.6.
  - Long descriptions wrap cleanly within the card width.
- **Dependencies**: T3.1, T0.5.

---

## Epic 4 — Run Execution Core

The heart. Unblocks live and historical run views.

### T4.1 [backend] EventParser

- **Description**: Stateful heuristic streaming parser for interactive Claude CLI stdout. Accepts byte chunks; emits `RunEvent`s using the pattern set documented in KB §6.7. Buffers partial lines. Patterns must be validated against actual Claude CLI output before this task is considered complete (see KB §9 item 1).
- **Acceptance**:
  - Unit tests cover: single chunk one event; one event split across many chunks; multiple events in one chunk; unknown line -> `AssistantText` fallback; malformed block body -> `system` event with raw bytes.
  - First parsed event is emitted within 250ms of receiving the first CLI output byte (NFR-5).
  - Patterns are loaded from `runs/parser/patterns.rs` with a logged version constant at run start.
  - Parser is allocation-conscious (no per-byte allocs).
- **Dependencies**: T0.2.

### T4.2 [backend] TranscriptWriter

- **Description**: Per-run writer task. Owns three file handles: `meta.json`, `transcript.jsonl`, `raw.log`. Atomic `meta.json` updates (tmp + rename). JSONL appended with a flush per line. Raw bytes appended to `raw.log` unchanged.
- **Acceptance**:
  - Killing the process mid-run leaves a valid (but partial) `transcript.jsonl` (no truncated line) and a `meta.json` that still parses.
  - Concurrent writers for different runs do not interfere.
- **Dependencies**: T0.2.

### T4.3 [backend] RunSession + RunManager

- **Description**: `RunManager` spawns Claude CLI as a child process with `cwd=project.path`. Manages a map of `run_id -> RunSession`. `RunSession` owns the child, parser, writer, cancellation token. Emits `run:started`, `run:event`, `run:finished`.
- **Acceptance**:
  - `launch_run` returns a `Run` with `status=Pending` within 200ms (NFR-4 visible feedback); `running` shortly after.
  - Stdin write via `send_input` reaches the child (validated with an echo binary in test).
  - Stop via `stop_run` kills the child, drains remaining output, finalizes meta.json with `status=stopped`.
  - Two simultaneous runs in the same project do not collide on transcript files (separate run-id dirs).
- **Dependencies**: T4.1, T4.2, T1.2.

### T4.4 [backend] Attached-md context handling

- **Description**: When `attached_md_path` is set on `LaunchInput`, prepend the file's content to the first stdin write (or as a CLI arg if Claude CLI supports it — Coder picks based on T1.2 probe). Record the path in `meta.json`.
- **Acceptance**:
  - File contents reach the child process.
  - Missing file at launch time -> `AppError::NotFound` before spawn.
- **Dependencies**: T4.3.

### T4.5 [backend] OrphanReaper

- **Description**: On app startup, scan all registered projects' `.claude/runs/*/meta.json`. For any with `status in (pending, running)`, check if PID is alive AND its exe path matches the configured claude CLI. If yes, kill it. Mark all such runs `failed` with `note="Terminated (app restarted)"`.
- **Acceptance**:
  - Smoke test: launch a run, force-quit the app, relaunch -> run is marked failed and the (mock) child is gone.
  - Conservative: a PID alive but with a different exe is NOT killed.
- **Dependencies**: T4.3, T2.1.

### T4.6 [backend] RetentionPruner

- **Description**: At startup and on a 24h timer, walk each project's runs dir. Apply both retention rules from settings (age days; total-size MB per project). Delete oldest first to satisfy both. Skip runs in any non-terminal state. Emit `info` log lines per deletion.
- **Acceptance**:
  - With 600MB of fake runs and a 500MB cap, oldest are pruned to bring it under.
  - With 31-day-old runs and a 30-day cap, those are pruned.
  - Active runs are never deleted.
- **Dependencies**: T4.3, T1.1.

### T4.7 [backend] respond_to_step_failure command + step-failure detection

- **Description**: Parser emits `StepFailed` event on detecting a step failure marker (markers per CLI integration — Coder pins during T4.1). `RunSession` also emits `run:step_failure`. `respond_to_step_failure(run_id, choice)` writes the appropriate token to stdin. A 60s timer auto-Continues if no response.
- **Acceptance**:
  - Mock CLI that emits a step-failure marker triggers the event.
  - Each of the four choices is dispatched per the protocol resolved in T4.8 (either via stdin token or via kill + re-invoke).
  - No response within 60s -> Continue is auto-sent and logged.
- **Dependencies**: T4.3, T4.8.

### T4.8 [backend] Research: step-failure interaction protocol

- **Description**: Before implementing Retry/Skip/Abort/Continue UI, verify whether Claude CLI (interactive mode) supports receiving specific stdin tokens to influence mid-run behavior, or whether "retry" must be implemented as: kill current subprocess + re-invoke the same step. Document findings in knowledge-base.md §9 and update T4.7 scope accordingly.
- **Acceptance**:
  - knowledge-base.md §9 updated with concrete finding: either (a) exact stdin tokens confirmed, or (b) "retry = new subprocess" approach documented with implementation plan.
  - T4.7 acceptance criteria updated to match.
- **Dependencies**: T4.1, T4.3 (need a running interactive session to test).
- **Note**: If stdin tokens don't work, the "Continue" action = re-invoke the step. "Retry" = same. "Skip" = advance to next step index. "Abort" = kill and mark failed. These can all be implemented without special stdin tokens.

---

## Epic 5 — Run UI (Live and Historical)

### T5.1 [frontend] EventBlock components

- **Description**: One component per `RunEvent` variant (KB §4). Each implements the visual treatment from UI §5.4 (assistant markdown, thinking collapsible, tool call expandable, tool result, file edit diff, user input bubble, system, step failed).
- **Acceptance**:
  - Storybook-style harness page renders one of each, against fixture data committed to the repo.
  - Markdown sanitization in AssistantBlock (no script injection from CLI output).
  - DiffBlock highlights additions green, deletions red, context neutral.
- **Dependencies**: T0.5.

### T5.2 [frontend] liveRuns Zustand store + useLiveRun hook

- **Description**: Store keyed by `run_id` holds: events array, status, started_at, exit_code. `useLiveRun(run_id)` subscribes to `run:event`, `run:started`, `run:finished` filtered by id. Bounded buffer (10k events) with overflow log.
- **Acceptance**:
  - Mounting/unmounting a live run view does not leak event listeners.
  - Two live run views open simultaneously remain isolated.
- **Dependencies**: T0.4, T4.3.

### T5.3 [frontend] S-04 Run View Live

- **Description**: Implement UI §5.4. Header bar with Stop (inline confirm), event stream with auto-scroll + jump-to-bottom, persistent UserInputBox (Enter sends, Shift+Enter newline).
- **Acceptance**:
  - All states from UI §5.4 render correctly (pending, running, stopping, completed, failed, stopped).
  - UserInputBox sends via `send_input`; on terminal state, input box and Send disable.
  - Auto-scroll pauses on user scroll-up; Jump-to-bottom appears and works.
- **Dependencies**: T5.1, T5.2, T4.3.

### T5.4 [frontend] Step failure prompt UI

- **Description**: Inline within the event stream, on `run:step_failure` event, render a card with Retry / Skip / Abort / Continue (default highlighted). Clicking sends `respond_to_step_failure`. Auto-dismisses after 60s as Continue (with countdown indicator).
- **Acceptance**:
  - Buttons send the correct choice.
  - 60s countdown is visible.
  - Auto-Continue fires if untouched.
- **Dependencies**: T5.3, T4.7.

### T5.5 [backend] load_transcript + list_runs commands

- **Description**: `list_runs(project_id)` returns `Vec<Run>` from `meta.json` files, sorted newest-first. `load_transcript(run_id, project_id)` streams `transcript.jsonl` and returns the full event list (or an error if missing/corrupt).
- **Acceptance**:
  - 1000-event transcript loads in <500ms on a mid-range dev machine.
  - Corrupt JSONL line -> error variant `ParseError` with line number.
- **Dependencies**: T4.2.

### T5.6 [frontend] S-05 Run View Historical

- **Description**: Reuses `EventBlock` components from T5.1. Loads transcript via `load_transcript`. No Stop button, no UserInputBox, shows duration. Error state with [Open folder] when transcript unavailable.
- **Acceptance**:
  - Renders a completed run identically to its live view.
  - Open folder reveals the run dir in OS file manager.
- **Dependencies**: T5.1, T5.5.

### T5.7 [frontend] S-03 Project Detail

- **Description**: UI §5.3. Header with Launch Sequence button, git status bar, two-panel layout: Run History (uses `list_runs`) and Sequences (uses `list_sequences`). Sequences panel focus-and-pulse mode when entered from quick-run with no prior run.
- **Acceptance**:
  - Clicking a run row routes to S-04 (if still running) or S-05.
  - Sequences panel highlight pulse runs for 2s and fades.
  - Refresh icon in git status bar calls `refresh_git_status`.
- **Dependencies**: T3.2, T5.5, T2.6.

### T5.8 [frontend] S-06 Launch Modal

- **Description**: UI §5.6. Sequence selector + optional .md attach. Pre-fill when entered from quick-run with prior run. Launch button calls `launch_run` and navigates to the live run view.
- **Acceptance**:
  - Backdrop click and Esc close the modal.
  - Attached file chip shows filename; × removes attachment.
  - Launch failure shows the error banner without closing the modal.
- **Dependencies**: T3.2, T4.3.

### T5.9 [frontend] Background run badge on project card

- **Description**: When `run:event`/`run:started`/`run:finished` events arrive for runs whose project is visible, the card shows a "Running" badge (or "N running" if >1). Clicking the badge navigates to the live run view (or shows a small picker if multiple).
- **Acceptance**:
  - Starting a run while on the Dashboard updates the card's badge within 500ms.
  - Multiple runs on the same project show the count and the picker.
- **Dependencies**: T5.2, T2.5.

---

## Epic 6 — Toasts and Notifications

### T6.1 [frontend] toasts store + Toast component

- **Description**: Zustand store with a bounded toast queue (max 4 visible, FIFO). Toast component per UI §5.9 with progress bar for timed dismissal.
- **Acceptance**:
  - Success/Stopped toasts auto-dismiss in 8s; failed persist until dismissed.
  - Multiple toasts stack and animate per UI §5.9.
- **Dependencies**: T0.5.

### T6.2 [backend] toast:show on run terminal events

- **Description**: On `run:finished`, emit `toast:show` with kind=completed/failed/stopped, the sequence + project names, and the `run_id`.
- **Acceptance**:
  - Each terminal state emits exactly one toast.
- **Dependencies**: T4.3, T6.1.

### T6.3 [frontend] toast click navigation

- **Description**: Clicking a toast routes to the run view (S-05 if terminal, S-04 if still accessible).
- **Acceptance**:
  - Routing works from any screen.
- **Dependencies**: T6.1, T5.6.

---

## Epic 7 — Usage / Rate Limit

### T7.1 [backend] UsageProbe

- **Description**: Runs `<claude> /usage` as a subprocess (short timeout, 10s). Parses key-value lines from stdout into a `BTreeMap`. Schedules every 60s + on-demand. Pauses on window blur. Emits `usage:updated`.
- **Acceptance**:
  - Mock CLI returning known KV output -> snapshot reflects keys.
  - Subprocess failure -> `available=false`, error logged, no exception.
- **Dependencies**: T1.2.

### T7.2 [frontend] RateLimitPill + useUsage hook

- **Description**: Pill in Dashboard top bar (UI §5.2). Clicking opens a popover with full KV list and a Refresh button calling `refresh_usage`.
- **Acceptance**:
  - "--" state when `available=false`.
  - Spinner during refresh.
  - Updates within 500ms of `usage:updated` event.
- **Dependencies**: T7.1, T2.6.

---

## Epic 8 — Polish and Cross-Cutting

### T8.1 [shared] Error translation utility

- **Description**: `utils/errors.ts` maps `AppError` `code`s to user-facing strings. Components display via `formatError(err)` only.
- **Acceptance**:
  - Every `AppError` variant has a translation.
  - Unknown codes fall back to a generic "Something went wrong (CODE: XYZ)".
- **Dependencies**: T0.2.

### T8.2 [frontend] Empty/loading/error states audit

- **Description**: Sweep every screen and component for the states enumerated in UI §7. Add missing skeletons, empty states, and error states.
- **Acceptance**:
  - Each row in the UI §7 table is reproducible by flipping a fixture flag.
- **Dependencies**: all major UI tasks.

### T8.3 [backend] Window focus/blur bridge

- **Description**: Wire Tauri window focus/blur/show/hide events to the `GitPoller`, `UsageProbe`, and `cli:lost` checker so they pause/resume.
- **Acceptance**:
  - Polling logs show no activity while window is blurred.
  - Resuming on focus triggers an immediate poll.
- **Dependencies**: T2.3, T7.1, T1.6.

### T8.4 [backend] Graceful shutdown

- **Description**: On window close, send cancel to all RunSession tokens. Wait up to 2s for cleanup, then force-exit. Finalize meta.json for any still-running runs as `failed` with note "App shutdown".
- **Acceptance**:
  - Closing during an active run leaves a coherent meta.json + transcript.jsonl on next launch.
- **Dependencies**: T4.3.

### T8.5 [frontend] Keyboard shortcuts and accessibility pass

- **Description**: Esc closes modals; Enter submits primary action; focus rings visible (UI tokens); reduced-motion respected. Tab order audited on each screen.
- **Acceptance**:
  - All modals dismissable via Esc.
  - No focus traps.
  - Reduced-motion disables all transitions.
- **Dependencies**: T5.8, T2.7.

### T8.6 [shared] README and developer onboarding

- **Description**: Top-level README: prerequisites, install, dev, build, test, troubleshooting. Brief architecture pointer to KB.
- **Acceptance**:
  - A fresh dev can clone and reach a running app in <10 minutes following the README.
- **Dependencies**: T0.1.

### T8.7 [shared] NFR verification smoke test

- **Description**: Manual verification of NFR-4 (visible feedback within 200ms of launch_run), NFR-5 (parsed events appear within 250ms of CLI emission), NFR-6 (idle RAM <200MB, CPU <1% with 20 projects). Document pass/fail results. Gate v1 release on PASS.
- **Acceptance**:
  - T4.3's acceptance criteria explicitly asserts run launch feedback <200ms.
  - T4.1's acceptance criteria asserts first parsed event emitted within 250ms of receiving first CLI output byte.
  - T8.7 manual checklist completed and checked in.
- **Dependencies**: T4.1, T4.3, T5.3 (all streaming tasks complete).

---

## Epic 9 — Observability

Local structured logging and in-app health signals. No remote telemetry (NFR-8). Concrete schema lives in `.claude/monitoring.md` — Coder must follow that doc field-for-field.

### T9.1 [backend] IPC instrumentation middleware

- **Description**: Add an `instrument(name, |cid| async { ... })` helper in `src-tauri/src/ipc/commands.rs` that wraps every `#[tauri::command]`. It generates a `correlation_id` (uuid v4), opens `info_span!("ipc", command, correlation_id)`, captures `elapsed_ms` on close, and on `Err(AppError)` emits `tracing::error!(kind, correlation_id, command, message, "command failed")` AND attaches `correlation_id` to `AppError::details` so the frontend toast can display it. Non-streaming commands additionally emit a `warn!("slow command", elapsed_ms)` when `elapsed_ms > 500` (exclude `launch_run`, `load_transcript`). See monitoring.md §1.3.e and §2.2.
- **Acceptance**:
  - Every `#[tauri::command]` body is `instrument("name", |cid| async move { ... }).await`.
  - Running any command produces a JSON log line with `span.command=<name>` and an `elapsed_ms` field on the close event.
  - Forcing an `AppError::NotFound` yields one `"command failed"` error log AND the frontend receives `details.correlation_id` matching the log.
  - `cargo test --test ipc_log_fields` asserts each expected field is present.
- **Dependencies**: T0.6, T0.2.

### T9.2 [backend] Run, transcript, and parser logging

- **Description**: Apply the exact field set from monitoring.md §2.3, §2.4, §2.5 to `runs/session.rs`, `runs/parser.rs`, `runs/transcript.rs`. Includes: `run started` (with `spawn_latency_ms`, `pid`, `cli_path`), `run finished` (with `status`, `exit_code`, `duration_ms`, `events_emitted`, `bytes_in`), `spawn failed` error, parse-warning lines with `line_no`/`bytes_dropped`/`snippet`, slow-flush warns over 100 ms, and `transcript opened` info. Per-batch parse spans use `info_span!("parse_batch", run_id, bytes_in)` and only emit at INFO when `elapsed_ms > 50`.
- **Acceptance**:
  - A test run produces `"run started"` and `"run finished"` log events with all required fields.
  - Feeding a malformed line to `EventParser` produces a `warn` log with `kind="parse_error"` and a 200-char snippet, and a `System` event is appended to the transcript.
  - Forcing an `io::Error` on transcript write produces a `error` log with `kind="io"` and propagates `AppError::Io`.
- **Dependencies**: T9.1, T4.1, T4.2, T4.3.

### T9.3 [backend] Background-task logging (git, usage, retention, orphan, sequence, registry, settings)

- **Description**: Apply monitoring.md §2.6 - §2.12 to `projects/git.rs`, `usage.rs`, `runs/retention.rs`, `runs/orphan.rs`, `sequences/mod.rs`, `projects/mod.rs`, `settings.rs`. Includes: `git_poll` span with `git_error_class` classification on failure, slow-poll warn over 1000 ms, `usage_probe` span with `keys_parsed` on success and stderr_tail on failure, `usage snapshot stale` warn rate-limited to once per minute, per-prune `"run pruned"` info lines with `reason=age|size`, orphan-reap summary + per-kill lines.
- **Acceptance**:
  - With a fake repo that errors out, the log shows a `warn` with `kind="git"`, `git_error_class="repo_missing"`.
  - With a 600 MB / 500 MB cap fixture (re-using T4.6 test), the retention summary log shows `runs_pruned > 0` and per-deletion lines.
  - With a mock CLI returning non-zero, `usage probe failed` log includes `exit_code` and a non-empty `stderr_tail`.
- **Dependencies**: T9.1, T2.3, T4.5, T4.6, T7.1.

### T9.4 [shared] Frontend error pipe and correlation_id surfacing

- **Description**: Add a React root error boundary in `src/App.tsx` that calls `log_frontend_error({ message, stack, route })`. Update the existing `log_frontend_error` backend handler to emit the exact log shape from monitoring.md §1.3.k (`component="frontend"`, `kind="frontend"`, generated `correlation_id`, included `route` and `stack`). Update `src/utils/errors.ts` (built in T8.1) so any `AppError` whose `details.correlation_id` is set renders a small "CID: 8 chars" chip in the toast body and in the run-failure-toast body. Add a "Copy" button on the chip.
- **Acceptance**:
  - Throwing inside a React component triggers exactly one `error` log line with `component="frontend"` and a stack.
  - A command that returns an `AppError` shows a toast whose body includes the CID chip; clicking Copy puts the full UUID on the clipboard.
  - The same CID appears in `dev-dashboard.<date>.log` under the original `"command failed"` line.
- **Dependencies**: T9.1, T6.1, T8.1.

### T9.5 [backend] EnvFilter, log retention sweep, and Settings tooltip

- **Description**: In `init_tracing()`, replace any static level with `EnvFilter::try_from_env("DEV_DASHBOARD_LOG").unwrap_or_else(|_| EnvFilter::new("info"))` so users can target a module (e.g. `info,dev_dashboard::runs::parser=debug`). On app startup (after subscriber init, before any business work), scan `<logs_dir>` and delete `dev-dashboard.*.log` files with mtime older than 7 days, logging a single `info` line with `removed` count and `bytes_freed`. Update the Settings screen (T1.4) "Open logs folder" area with a short tooltip / help text showing the env-var syntax.
- **Acceptance**:
  - `DEV_DASHBOARD_LOG=warn` results in INFO lines being filtered out (verified by integration test asserting absence).
  - `DEV_DASHBOARD_LOG="info,dev_dashboard::runs::parser=debug"` produces parser DEBUG lines while keeping other modules at INFO.
  - A logs dir pre-seeded with 10 daily files, half older than 7 days, results in only the recent half remaining after startup, plus a single summary log line.
  - Settings screen shows the tooltip text and links to the logs folder.
- **Dependencies**: T0.6, T1.4.

---

## Dependency Graph (high-level)

```
T0.1
 |--T0.2--T0.3--T0.4
 |    |    |
 |    |    +--T0.6
 |    |
 |--T0.5
 |
 +--Epic 1 (T1.x)  -- unblocks Setup, Settings
 +--Epic 2 (T2.x)  -- unblocks Dashboard
 +--Epic 3 (T3.x)  -- unblocks Project Detail
 +--Epic 4 (T4.x)  -- core: parser/manager/writer/orphan/retention
 +--Epic 5 (T5.x)  -- depends on Epic 4 + Epic 3
 +--Epic 6 (T6.x)  -- depends on T4.3
 +--Epic 7 (T7.x)  -- depends on T1.2
 +--Epic 8 (T8.x)  -- polish, depends on Epics 2-7
 +--Epic 9 (T9.x)  -- observability, depends on T0.6 + Epics 2/4/6/7
```

## Parallelization Notes

After Epic 0 lands:

- **Track A (backend)**: T1.1 -> T1.2 -> T2.1 -> T2.2 -> T2.3 -> T3.1 -> T4.1 -> T4.2 -> T4.3 -> T4.8 -> T4.4/T4.5/T4.6/T4.7 -> T5.5 -> T6.2 -> T7.1 -> T8.3/T8.4 -> T8.7.
- **Track B (frontend)**: T1.3 -> T1.4 -> T2.4 -> T2.5 -> T2.6 -> T2.7 -> T3.2 -> T5.1 -> T5.2 -> T5.3 -> T5.4 -> T5.6 -> T5.7 -> T5.8 -> T5.9 -> T6.1 -> T6.3 -> T7.2 -> T8.1/T8.2/T8.5.

Frontend track can develop against typed mocks for any backend command before its Rust impl lands — the IPC contract is the seam.

---

## Coverage Check

Every requirement from `requirements.md` and every screen from `ui-ux-spec.md` is covered:

| Req / Screen | Task(s) |
|---|---|
| FR-1.1 register project | T2.1, T2.6 |
| FR-1.2 remove project | T2.1, T2.7 |
| FR-1.3 card display | T2.5, T2.6 |
| FR-1.4 git polling | T2.3, T8.3 |
| FR-1.5 missing state | T2.1, T2.5 |
| FR-2.1 sequences as files | T3.1 |
| FR-2.2 browse sequences | T3.1, T3.2 |
| FR-2.3 launch | T4.3, T5.8 |
| FR-2.4 attach .md | T4.4, T5.8 |
| FR-2.5 unlimited concurrency | T4.3 |
| FR-3.1 spawn CLI | T4.3 |
| FR-3.2 CLI missing | T1.2, T1.3 |
| FR-3.3 parsed rendering | T4.1, T5.1, T5.3 |
| FR-3.4 stop | T4.3, T5.3 |
| FR-3.5 orphan detection | T4.5 |
| FR-3.6 state transitions | T4.3 |
| FR-4.1/4.2 transcript persistence | T4.2 |
| FR-4.3 browse past runs | T5.5, T5.7 |
| FR-4.4 retention | T4.6, T1.4 |
| FR-5.1-5.4 toasts | T6.1, T6.2, T6.3 |
| FR-6.1/6.2 settings | T1.1, T1.4 |
| Step failure handling | T4.7, T5.4 |
| Usage status bar | T7.1, T7.2 |
| S-01 Setup | T1.3 |
| S-02 Dashboard | T2.5, T2.6, T2.7, T7.2 |
| S-03 Project Detail | T5.7 |
| S-04 Run Live | T5.3, T5.4, T5.9 |
| S-05 Run Historical | T5.6 |
| S-06 Launch Modal | T5.8 |
| S-07 Settings | T1.4 |
| S-08 Tag Editor | T2.7 |
| S-09 Toasts | T6.1, T6.2, T6.3 |
