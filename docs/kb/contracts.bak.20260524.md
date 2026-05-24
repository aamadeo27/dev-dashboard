# Contracts

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
