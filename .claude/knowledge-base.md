# Knowledge Base v1: Dev Dashboard

**Project**: Dev Dashboard
**Date**: 2026-05-18
**Status**: v1 — handoff to coders
**Audience**: all downstream agents (Coder, Tester, Monitor, Reviewer)

This document is the single shared reference for the architecture. Every other agent should treat it as authoritative. If it conflicts with Requirements or UI/UX, fix this document — do not silently diverge.

---

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

## 2. Stack Decisions

### 2.1 Tauri

- **Tauri 2.x** (latest stable as of 2026-05). v2 has stable cross-platform notification, FS, dialog, and shell plugins; sidecar/process management is well-supported.
- Why Tauri (vs Electron): smaller footprint (NFR-6: <200 MB RAM idle), native FS and process control from Rust, no bundled Chromium — matches NFR-2 (no exposed local server beyond loopback IPC).

### 2.2 Frontend: React + TypeScript + Vite

- **Framework**: **React 18 + TypeScript**, built with **Vite**.
- **Justification**:
  - Single developer + Sonnet coders: React has the largest training corpus, fewest surprise idioms.
  - Component library reuse: the UI spec has a clear component inventory (`ProjectCard`, `EventBlock`, etc.) — React's component model maps 1:1.
  - Streaming-heavy run view benefits from `useSyncExternalStore` + memoization patterns that are well-trodden in React.
  - TS is non-negotiable for the IPC contract surface (Rust types -> TS types via `ts-rs` or hand-mirrored).
- **Rejected**: Svelte (smaller ecosystem for the dev tools we need; coder unfamiliarity tax); Vue (no clear win over React here); SolidJS (too niche for a Sonnet coder to navigate confidently).

### 2.3 State management: Zustand + React Query (TanStack Query)

- **Zustand** for client-only UI state (modals open, selected project, scroll positions, toast queue, draft text in input boxes).
- **TanStack Query** for everything that originates in Rust (projects list, git status, sequences, run history, usage snapshot). Cache + invalidation handles the polling cases cleanly.
- **Live run events**: a dedicated Zustand store per active run keyed by `run_id`, populated from Tauri event subscriptions. Not in TanStack Query — these are push, not pull.
- **Rejected**: Redux Toolkit (overkill, more boilerplate than the project warrants); Context-only (re-render storms in run view).

### 2.4 IPC pattern

Two channels, both Tauri-native:

1. **Commands (request/response)**: frontend calls a typed Rust function, awaits a `Result`. Used for CRUD, launch, stop, settings, etc.
2. **Events (push)**: Rust emits to the webview using `window.emit`. Used for streaming run events, git status updates, usage refreshes, toast triggers.

Event channel naming: `<domain>:<action>`, e.g. `run:event`, `run:finished`, `git:updated`, `project:missing`, `usage:updated`, `toast:show`. Payloads are always typed JSON.

For run events, the frontend subscribes once per mounted run view to `run:event` with a payload filter `{ run_id }`. The Rust side does **not** demux per-listener — it emits all events; frontend filters. Simpler, and N is bounded by visible run views.

### 2.5 Build tooling

- **Vite** for the web side. Dev server only used in `tauri dev`.
- **Cargo** for the Rust side, workspace not needed (single crate sufficient at this size; can split later if `RunManager` grows).
- **pnpm** for JS package management (faster than npm, deterministic).
- **Biome** for JS/TS lint + format (single tool, no ESLint+Prettier config churn).
- **rustfmt + clippy** on Rust, gated in CI/local pre-commit.

### 2.6 Key Rust crates

| Crate | Use |
|---|---|
| `tauri` 2.x | App framework |
| `tauri-plugin-dialog` | Native file/directory pickers |
| `tauri-plugin-opener` | Open project paths in OS default editor / file manager / terminal (URLs and paths) |
| `tauri-plugin-fs` | (sparingly) FS access from frontend if needed; prefer commands |
| `tokio` | Async runtime, processes, timers |
| `git2` | Libgit2 bindings for git status |
| `serde`, `serde_json` | Serialization, JSONL |
| `chrono` | Timestamps, durations |
| `uuid` | Run IDs (v7 — time-sortable) |
| `dirs` | OS-standard config directory |
| `tracing`, `tracing-subscriber`, `tracing-appender` | Structured logging |
| `thiserror` | Error types |
| `notify` (optional) | FS watch for sequences directory (mtime fallback if too costly) |
| `sysinfo` | Orphan reaper: check if PID is alive and matches expected exe name |
| `ts-rs` | Auto-generate TS bindings from Rust structs |

**Plugin choice — `plugin-opener` vs `plugin-shell`**: we use `tauri-plugin-opener` (not `tauri-plugin-shell`) for the "Open in Editor" and "Open in Terminal" context-menu actions. `plugin-opener` is the Tauri 2 plugin specifically designed to hand a path/URL to the OS's default handler — which is exactly what these actions need. `plugin-shell` is for spawning and managing arbitrary child processes with stdin/stdout control; using it here would require a broader allowlist than necessary and exposes capabilities (arbitrary command execution from the webview) we do not want. `plugin-shell` is not used at all in v1: Claude CLI subprocesses are spawned directly via `tokio::process::Command` from the Rust core (not via the shell plugin), since they need stdin piping and stream parsing that the plugin does not provide.

### 2.7 Key JS packages

| Package | Use |
|---|---|
| `react`, `react-dom` | UI |
| `@tauri-apps/api` v2 | Command + event IPC |
| `@tauri-apps/plugin-dialog` | File pickers |
| `@tauri-apps/plugin-opener` | Open in editor/terminal (OS default app for path/URL) |
| `zustand` | Client state |
| `@tanstack/react-query` | Server-state cache |
| `react-router-dom` | Screen routing (S-01 through S-07) |
| `lucide-react` | Icons (smaller bundle, better TS types than Phosphor) |
| `react-markdown` + `remark-gfm` | Render assistant text |
| `diff2html` | Unified diff rendering for file-edit events (MIT licensed, richer rendering than `diff`) |
| `clsx` | Class composition |

---

## 3. Data Model

All entities below are defined in Rust and exported to TS via `ts-rs`. The Rust struct is the source of truth.

### 3.1 `Project`

```rust
struct Project {
    id: String,             // UUID v7
    name: String,           // basename of path, user-editable
    path: PathBuf,          // absolute, canonicalized
    tags: Vec<String>,      // lowercased, trimmed, deduped
    language: Option<String>,    // detected: "rust", "ts", "python", ...
    package_manager: Option<String>, // "cargo", "pnpm", "npm", "uv", ...
    added_at: DateTime<Utc>,
    last_modified: Option<DateTime<Utc>>, // mtime of project root
    is_missing: bool,       // computed; not persisted
}
```

**Storage**: `<os_config_dir>/dev-dashboard/projects.json`.

### 3.2 `Sequence`

```rust
struct Sequence {
    name: String,        // filename minus ".md"
    description: String, // first non-heading paragraph; "(No description)" fallback
    path: PathBuf,       // absolute path to the .md file
    mtime: DateTime<Utc>,// for cache invalidation
}
```

**Storage**: filesystem only at `<project>/.claude/sequences/*.md`. Loaded on demand, cached in-memory keyed by project_id.

### 3.3 `Run`

```rust
struct Run {
    id: String,                  // UUID v7
    project_id: String,
    project_path: PathBuf,       // captured at launch time (project may move)
    sequence_name: String,
    attached_md_path: Option<PathBuf>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    status: RunStatus,           // pending|running|completed|failed|stopped
    exit_code: Option<i32>,
    pid: Option<u32>,
    note: Option<String>,        // e.g. "Terminated (app restarted)"
}

enum RunStatus { Pending, Running, Completed, Failed, Stopped }
```

**Storage**: `<project>/.claude/runs/<run-id>/meta.json`. Written on state change.

### 3.4 `RunEvent`

```rust
#[serde(tag = "type", rename_all = "snake_case")]
enum RunEvent {
    AssistantText { text: String, ts: DateTime<Utc> },
    Thinking      { text: String, ts: DateTime<Utc> },
    ToolCall      { id: String, name: String, input: serde_json::Value, ts: DateTime<Utc> },
    ToolResult    { call_id: String, output: serde_json::Value, is_error: bool, ts: DateTime<Utc> },
    FileEdit      { path: String, diff: String, additions: u32, deletions: u32, ts: DateTime<Utc> },
    UserInput     { text: String, ts: DateTime<Utc> },
    System        { text: String, ts: DateTime<Utc> },
    StepFailed    { step: String, message: String, ts: DateTime<Utc> },
    Error         { message: String, ts: DateTime<Utc> },
}
```

**Storage**: `<project>/.claude/runs/<run-id>/transcript.jsonl`. One event per line. Append-only.

A second file, `raw.log`, captures unmodified stdout+stderr bytes for debugging.

### 3.5 `Settings`

```rust
struct Settings {
    parent_dir: Option<PathBuf>,     // GAP-07: single configured parent dir
    claude_cli_path: Option<PathBuf>,// overrides PATH lookup
    git_poll_interval_secs: u32,     // default 10, min 5, max 3600
    usage_poll_interval_secs: u32,   // default 60, min 30, max 3600
    retention_days: u32,             // default 30, min 1
    retention_size_mb: u32,          // default 500, min 50
    view_mode: ViewMode,             // Grid | List
}
```

**Storage**: `<os_config_dir>/dev-dashboard/settings.json`.

### 3.6 `UsageSnapshot`

```rust
struct UsageSnapshot {
    fetched_at: DateTime<Utc>,
    parsed: BTreeMap<String, String>, // ordered key-value parse of `claude /usage` stdout
    raw_stdout: String,
    available: bool,                  // false if subprocess failed
}
```

**Storage**: in-memory only. Re-fetched on app start; not persisted.

### 3.7 Auth

**Decision**: **No authentication.** The app is local single-user (NFR-2). It binds nothing on the network. The only credential boundary is the OS user account. No login screen, no session token, no encryption-at-rest beyond OS filesystem permissions (config dir is per-user by default).

**Rationale**: adding auth here would be friction without security benefit. The threat model is "another user on the same machine reads my files" — and that is the OS's job. We do not need to re-implement it.

---

## 4. File Layout (Codebase)

```
dev-dashboard/
├── .claude/                       # this project's own dashboard data (dogfood)
│   ├── requirements.md
│   ├── ui-ux-spec.md
│   ├── knowledge-base.md          # THIS FILE
│   └── epics.md
├── src-tauri/                     # Rust side
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs                # thin binary entrypoint; calls `dev_dashboard_lib::run()`
│       ├── lib.rs                 # library crate root: `pub fn run()`, plugin registration, command list, module wiring
│       ├── app_state.rs           # AppState struct (held in tauri::State)
│       ├── error.rs               # AppError + AppResult
│       ├── settings.rs            # SettingsStore
│       ├── projects/
│       │   ├── mod.rs             # ProjectRegistry
│       │   ├── scanner.rs         # language/pm detection
│       │   └── git.rs             # GitPoller, git status via git2
│       ├── sequences/
│       │   └── mod.rs             # SequenceLoader, description extraction
│       ├── runs/
│       │   ├── mod.rs             # RunManager
│       │   ├── session.rs         # RunSession (per-run state)
│       │   ├── parser.rs          # EventParser (stream -> RunEvent)
│       │   ├── transcript.rs      # TranscriptWriter (JSONL + raw)
│       │   ├── orphan.rs          # OrphanReaper
│       │   └── retention.rs       # RetentionPruner
│       ├── usage.rs               # UsageProbe
│       ├── ipc/
│       │   ├── mod.rs             # command registration
│       │   ├── commands.rs        # all #[tauri::command] fns
│       │   └── events.rs          # event name constants + emit helpers
│       └── platform/
│           ├── editor.rs          # open-in-editor
│           └── terminal.rs        # open-in-terminal
├── src/                           # Frontend (React + TS)
│   ├── main.tsx
│   ├── App.tsx                    # router + global providers
│   ├── routes/
│   │   ├── Setup.tsx              # S-01
│   │   ├── Dashboard.tsx          # S-02
│   │   ├── ProjectDetail.tsx      # S-03
│   │   ├── RunLive.tsx            # S-04
│   │   ├── RunHistorical.tsx      # S-05
│   │   └── Settings.tsx           # S-07
│   ├── components/
│   │   ├── ProjectCard.tsx
│   │   ├── GitStatusBadge.tsx
│   │   ├── RunOutcomeBadge.tsx
│   │   ├── EventBlock/            # one file per event type
│   │   │   ├── AssistantBlock.tsx
│   │   │   ├── ThinkingBlock.tsx
│   │   │   ├── ToolCallBlock.tsx
│   │   │   ├── ToolResultBlock.tsx
│   │   │   ├── FileEditBlock.tsx
│   │   │   ├── UserInputBlock.tsx
│   │   │   ├── SystemBlock.tsx
│   │   │   └── StepFailedBlock.tsx
│   │   ├── LaunchModal.tsx        # S-06
│   │   ├── TagEditorPopover.tsx   # S-08
│   │   ├── Toast.tsx              # S-09
│   │   ├── RateLimitPill.tsx
│   │   └── ContextMenu.tsx
│   ├── stores/
│   │   ├── ui.ts                  # zustand: modals, view mode, etc.
│   │   ├── toasts.ts              # zustand: toast queue
│   │   └── liveRuns.ts            # zustand: live run event buffers
│   ├── ipc/
│   │   ├── commands.ts            # typed wrappers around invoke()
│   │   ├── events.ts              # typed event subscribers
│   │   └── bindings.ts            # AUTO-GENERATED from ts-rs (do not edit)
│   ├── hooks/
│   │   ├── useProjects.ts
│   │   ├── useGitStatus.ts
│   │   ├── useSequences.ts
│   │   ├── useRunHistory.ts
│   │   ├── useLiveRun.ts
│   │   ├── useUsage.ts
│   │   └── useSettings.ts
│   ├── styles/
│   │   ├── tokens.css             # CSS variables from UI spec section 1.1
│   │   └── globals.css
│   └── utils/
│       ├── format.ts              # relative timestamps, durations, sizes
│       └── markdown.ts            # safe markdown rendering
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── biome.json
└── README.md
```

**Rust crate layout — `lib.rs` + thin `main.rs`**: `src-tauri` is configured as both a library crate (`dev_dashboard_lib`) and a binary crate. `main.rs` contains only `fn main() { dev_dashboard_lib::run() }`; `lib.rs` exposes `pub fn run()` which performs plugin registration, builds `AppState`, registers the command handlers, and runs the Tauri app. This is the standard Tauri 2 pattern: it allows `cargo test` to exercise domain modules without spawning the binary, supports integration tests under `src-tauri/tests/`, and is required for mobile targets (iOS/Android) should we ever add them. The `Cargo.toml` declares both `[lib]` (name `dev_dashboard_lib`, `crate-type = ["staticlib", "cdylib", "rlib"]`) and `[[bin]]` (name `dev-dashboard`, `path = "src/main.rs"`). No business logic lives in `main.rs`.

**Domain module visibility**: Domain modules (`projects`, `runs`, `sequences`, `settings`, `usage`) are declared `pub` in `lib.rs`. This is required for integration tests under `src-tauri/tests/` (which compile as separate crates and must see `pub` items) and for the ts-rs export tooling. The crate is `cdylib`/`rlib` with no external Rust consumers; this `pub` is Tauri-internal and does not constitute a stability contract — breaking changes within these modules are allowed without deprecation.

---

## 5. IPC Contracts (Commands)

All commands return `Result<T, AppError>`. `AppError` is a typed enum serialized as `{ code: string, message: string, details?: any }`.

### 5.1 Projects

```rust
list_projects() -> Vec<Project>
add_project(path: PathBuf) -> Project
remove_project(id: String) -> ()
relocate_project(id: String, new_path: PathBuf) -> Project
set_project_tags(id: String, tags: Vec<String>) -> Project
rename_project(id: String, name: String) -> Project
get_git_status(id: String) -> GitStatus
refresh_git_status(id: String) -> GitStatus     // forces immediate poll
set_visible_projects(ids: Vec<String>) -> ()    // GitPoller visible-set update (debounced)
open_in_editor(id: String) -> ()
open_in_terminal(id: String) -> ()
```

> `rename_project` and `delete_run` are **v1: internal-only. Not exposed via `commands.ts` frontend wrappers. No UI entry point in v1.** They are kept in the Rust surface to support future UI work without a contract change. (`delete_run` is defined in §5.3.)

`GitStatus`:

```rust
struct GitStatus {
    branch: Option<String>,
    is_clean: bool,
    dirty_files: u32,
    ahead: u32,
    behind: u32,
    last_polled: DateTime<Utc>,
    error: Option<String>,
}
```

### 5.2 Sequences

```rust
list_sequences(project_id: String) -> Vec<Sequence>
refresh_sequences(project_id: String) -> Vec<Sequence>   // bust cache
```

### 5.3 Runs

```rust
launch_run(input: LaunchInput) -> Run
stop_run(run_id: String) -> ()
send_input(run_id: String, text: String) -> ()
respond_to_step_failure(run_id: String, choice: StepFailureChoice) -> ()
list_runs(project_id: String) -> Vec<Run>                // newest first, meta.json only
get_run(run_id: String, project_id: String) -> Run
load_transcript(run_id: String, project_id: String) -> Vec<RunEvent>
delete_run(run_id: String, project_id: String) -> ()    // optional, manual prune

struct LaunchInput {
    project_id: String,
    sequence_name: String,
    attached_md_path: Option<PathBuf>,
}

enum StepFailureChoice { Retry, Skip, Abort, Continue }
```

### 5.4 Settings + system

```rust
get_settings() -> Settings
update_settings(patch: SettingsPatch) -> Settings
verify_claude_cli(path_override: Option<PathBuf>) -> CliCheck
get_usage() -> Option<UsageSnapshot>
refresh_usage() -> UsageSnapshot
open_logs_folder() -> ()
log_frontend_error(message: String, stack: Option<String>, route: Option<String>) -> ()

struct CliCheck { found: bool, resolved_path: Option<PathBuf>, version: Option<String>, error: Option<String> }
```

### 5.5 Events (Rust -> frontend)

| Event name        | Payload                                  | Frequency                       |
|---|---|---|
| `run:started`     | `{ run_id, project_id }`                 | once per run                    |
| `run:event`       | `{ run_id, event: RunEvent }`            | per parsed event                |
| `run:finished`    | `{ run_id, status, exit_code }`          | once per run                    |
| `run:step_failure`| `{ run_id, step, message }`              | when a step fails               |
| `git:updated`     | `{ project_id, status: GitStatus }`      | per poll cycle / on focus       |
| `project:missing` | `{ project_id }`                         | when scan detects missing dir   |
| `usage:updated`   | `{ snapshot: UsageSnapshot }`            | every 60s and on refresh        |
| `cli:lost`        | `{ error: string }`                      | when CLI disappears mid-session |
| `toast:show`      | `{ kind, title, body, run_id? }`         | run terminal events             |

Event names are constants in `src-tauri/src/ipc/events.rs` and `src/ipc/events.ts` — never inline strings.

---

## 6. Patterns and Conventions

### 6.1 Layering

- **Rust**: `ipc/commands.rs` is a thin shim. All real work lives in the domain modules (`projects/`, `runs/`, etc.). Commands take `tauri::State<AppState>` and forward to domain methods. No business logic in command bodies.
- **Frontend**: route components compose feature components. Feature components consume hooks. Hooks consume `ipc/commands.ts` and `ipc/events.ts`. No direct `invoke()` calls inside components.

### 6.2 Error handling

- One `AppError` enum in Rust with concrete variants:
  - `NotFound`, `AlreadyExists`, `Io`, `Git`, `Cli`, `ParseError`, `InvalidInput`, `PermissionDenied`, `Internal`.
- Each variant serializes to `{ code: "NOT_FOUND" | ..., message, details? }`.
- The frontend has a single error-translation utility (`utils/errors.ts`) that maps codes to user-facing strings. Components never display raw `error.message` directly — always through the translator.
- Background tasks (poller, pruner, parser) **never panic**. Errors are logged via `tracing` and surfaced as `system` events or toasts where user-relevant.

### 6.3 Async and cancellation

- Every long-running task (run session, git poller, usage probe) holds a `CancellationToken` (tokio-util). Stop = cancel token, then `child.kill()` for processes.
- Window close: graceful shutdown sends cancel to all tokens, waits up to 2 seconds, then aborts.

### 6.4 File encoding and line endings

- All transcripts, logs, settings, registry: **UTF-8, no BOM, LF line endings**, regardless of platform.
- Sequence `.md` files: read as UTF-8; fall back to UTF-8 lossy if invalid bytes (log a warning).
- Paths: `PathBuf` in Rust, `string` (absolute) in TS. Never relative across the IPC boundary.

### 6.5 Naming

- Rust: `snake_case` for fns/vars, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- TS: `camelCase` for vars/fns, `PascalCase` for types and React components.
- Tauri commands: `snake_case` on the wire (Rust default). TS wrappers mirror with `camelCase`.
- Files: kebab-case for non-component files (`format.ts`, `app-state.rs`), `PascalCase.tsx` for React components.

### 6.6 Testing approach

- **Rust**: unit tests inline (`#[cfg(test)]`) for parser, retention, orphan detection, settings (de)serialization. Integration tests in `src-tauri/tests/` for filesystem-touching flows using a `tempdir`.
- **Frontend**: Vitest for utility functions and event-block components (snapshot + interaction). React Testing Library for components. Avoid testing TanStack Query plumbing — trust the lib.
- **End-to-end**: deferred until v1.1. The Tauri E2E story (WebDriver) is fragile; manual smoke checklist suffices for v1.

### 6.7 Streaming parser strategy

**Decision**: launch `claude` in **interactive mode** (no `--print` flag, no `--output-format` flag). The parser is heuristic and reads the human-readable stdout that Claude emits in interactive sessions.

The Claude CLI's interactive stdout format is **the** load-bearing assumption. The parser must:
1. Buffer bytes until a newline (or `\r\n`) is seen.
2. Match each line against a set of known patterns to detect event boundaries.
3. Carry state across lines for multi-line constructs (thinking blocks, file diffs, tool inputs/outputs).
4. On no-match, emit an `AssistantText` event with the line as plain text — never drop input.
5. On internal parse error inside a recognized block, emit a `system` event with the raw line.

**Known Claude interactive output patterns to match** (subject to validation against the actual CLI build — see §9):
- Tool call header: lines beginning with `⏺ Tool: <name>` (the "⏺" sentinel marks an assistant action).
- Tool input block: indented JSON or key-value lines following a tool header until a dedent / blank line.
- Tool result: lines beginning with `⎿` (result-arrow sentinel) or `Result:` header.
- Thinking indicator: lines wrapped in `<thinking>` / `</thinking>` markers, or a leading `✻ Thinking` marker followed by indented content.
- File edit markers: a tool call to `Edit`/`Write` plus a fenced unified-diff block (`@@ ... @@` headers); the parser captures the diff body and counts additions/deletions.
- Assistant text: any line not matched by the above, after the first prompt is sent.
- Step failure: a known sentinel emitted by Claude when a step cannot continue (exact marker to be confirmed during T4.7 / §9 item 1).

**Parser implementation note**: keep pattern definitions in a single module (`runs/parser/patterns.rs`) so they can be updated as the CLI evolves. Each pattern carries a `Version` constant; the parser logs the active pattern-set version at run start for diagnostics. **These patterns must be validated against actual Claude CLI output before T4.1 is considered complete.**

### 6.8 Concurrency limits and backpressure

- No hard cap on concurrent runs (FR-2.5).
- Per-run event channel: `tokio::sync::mpsc` with capacity 1024. If full (slow frontend), drop **non-essential** events (assistant text only) and emit a `system` event "Event buffer overflow — UI lagged". Never drop tool calls, file edits, or terminal events.
- Transcript writes are serialized per-run (one writer task owns the file handle).

### 6.9 Step failure protocol

When the parser detects a step failure (`StepFailed` event):
1. Emit `run:event` with the `StepFailed` payload (UI renders inline).
2. Emit `run:step_failure` (UI surfaces the Retry/Skip/Abort/Continue prompt).
3. Wait for `respond_to_step_failure` command (with 60s default timeout -> auto-Continue).
4. Translate choice to stdin input expected by Claude CLI (exact tokens TBD by Coder during CLI integration).

---

## 7. Monitoring

This section sets direction only — the Monitor agent owns concrete queries, dashboards, and alert rules.

### 7.1 Level

**Basic**, locally-scoped. No remote telemetry (NFR-8). All monitoring data lives on the user's machine and is for **the user's own debugging**. No outbound sinks.

### 7.2 Tool family

- **Logging**: `tracing` + `tracing-appender` writing to `<os_config_dir>/dev-dashboard/logs/dev-dashboard.YYYY-MM-DD.log` (daily rotation, keep 7 days).
- **In-app log viewer**: deferred. v1: the user opens the file in their editor.
- **Frontend errors**: caught by a root error boundary -> logged via `console.error` and forwarded to Rust via a `log:frontend_error` command for inclusion in the rotated log file.

Log format: structured JSON per line (`tracing_subscriber::fmt::Layer::json()`), default level `info` for production, `debug` when `DEV_DASHBOARD_LOG=debug` env var is set.

### 7.3 Must-have signals

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

### 7.4 Operations needing timing instrumentation

Wrap in a `tracing::info_span!` with elapsed time logged on close:
- `RunManager::launch_run` (spawn -> first event)
- `EventParser::feed` (per-batch, only log if > 50 ms)
- `GitPoller::poll_one`
- `UsageProbe::fetch`
- `SequenceLoader::load_all`
- `RetentionPruner::run`
- `ProjectRegistry::load` and `save`

### 7.5 Error categories to track

`AppError` variants double as error taxonomy. Each `error!` log includes `kind = <variant_name>`. Monitor agent: counts by `kind` over time, surface the top three weekly.

### 7.6 Self-diagnosis affordances

- **About / Diagnostics screen** (deferred to v1.1, but plan for it): app version, CLI version, log file path button "Reveal in Finder/Explorer", config dir path, OS info, list of running PIDs.
- For v1, the user reaches the log file via Settings -> "Open logs folder" button (low-cost addition; include it).
- Every `error` log line includes a `correlation_id` that — when relevant — is also surfaced in the failing toast's body so the user can grep for it.

---

## 8. Out-of-Scope Confirmations (carry-over)

These are enumerated in Requirements section 6 and remain out-of-scope at the architecture level:
- Sequence authoring/editing in-app
- Pause/resume runs
- Concurrency limits / queueing
- OS-native notifications
- Cloud sync, remote access
- Cross-project run history view

Architecture choices intentionally do not preclude future work in these areas (e.g., the event stream design is compatible with pause/resume; the run storage layout is compatible with global aggregation), but no scaffolding for them ships in v1.

---

## 9. Open Items for the Coder

Items where the architecture defers to implementation discovery. Coder must validate and report back; document the resolution.

1. **EventParser heuristic patterns**: must be validated against actual interactive Claude CLI output before T4.1 is complete. Capture a representative session transcript, list each pattern in §6.7, and confirm a real-world example matches. Record the CLI version tested in `runs/parser/patterns.rs` as the pattern-set baseline.
2. **`claude /usage` stdout format**: verify the exact parseable shape; the data model assumes a `BTreeMap<String, String>` which is forgiving.
3. **Step-failure interaction protocol** (BLOCKER-02, T4.8): confirm whether interactive Claude CLI supports stdin tokens to influence mid-run behavior (Retry/Skip/Abort/Continue), or whether all four choices must be implemented as "kill subprocess + re-invoke the step". Document the concrete finding here: either (a) exact stdin tokens confirmed, or (b) "retry = new subprocess" approach documented with implementation plan. T4.7 acceptance is updated to match the outcome.
4. **PID matching on orphan reap**: confirm the process name reported by `sysinfo` matches `claude` (or the configured CLI path) across all three OSes. Conservative: only kill if PID is alive AND exe path matches the configured CLI path.

---

## 10. Branching and PR Pattern

Full details in `.claude/devops.md` §3 and §4. Summary for coders:

**Branch naming**: `feat/<task-id>-short-desc`, `fix/<task-id>-short-desc`, `chore/<desc>`, `docs/<desc>`, `refactor/<desc>`. Example: `feat/T0.1-scaffold`.

**Base branch**: `main` only. All branches cut from `main`, all PRs target `main`.

**Merge strategy**: squash-merge. One commit per task on `main`.

**PR title format**: `<type>(<scope>): <imperative description>` — e.g. `feat(runs): implement RunManager spawn and session lifecycle`.

**PR body must include**: What (one sentence), How (2-4 bullets of key decisions), Test (what was verified), Checklist (`lint`, `typecheck`, `clippy`, `tests`, `bindings` if Rust types changed).

**Required checks before merge** (enforced by branch protection):
- `Check (ubuntu-latest)` — Biome lint, tsc, rustfmt, clippy, vitest, cargo test, bindings freshness
- `Check (macos-latest)` — same
- `Check (windows-latest)` — same

**Commit format**: Conventional Commits — `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `perf`, `build`. Subject: imperative, max 72 chars. Body explains *why*.

**No force-push to `main`**.

---

## 11. Secrets

Full details in `.claude/devops.md` §1.

**This repo has no secrets.** The application manages zero credentials. The `claude` CLI handles its own authentication externally and independently of this codebase.

- No `.env` files, no secret manager, no credentials in CI.
- Nothing to inject at build time or runtime.
- No tokens, API keys, or passwords are stored, referenced, or passed through this app.
- Hard rules (apply even if this changes in future):
  - No secrets in git (not even gitignored `.env` with real values committed once).
  - No secrets in log output.
  - No secrets in client bundles or IPC payloads.
- Local dev story: not applicable — there is nothing to configure. Run `pnpm dev` and the app works.
