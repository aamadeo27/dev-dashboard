/// Tauri event name constants emitted by the backend.
/// Frontend counterparts are in `src/ipc/events.ts`.
pub const CLI_LOST: &str = "cli:lost";

/// Emitted after each git poll cycle for a single project.
///
/// Payload shape: `{ id: string, status: GitStatus }`.
/// Frontend counterpart: `GIT_UPDATED` in `src/ipc/events.ts`.
pub const GIT_UPDATED: &str = "git:updated";

/// Event emitted to display a toast notification in the frontend.
///
/// Payload shape (KB §5.5): `{ kind, title, body, run_id? }`
/// where `kind` is one of `"error"`, `"warning"`, `"info"`, `"success"`.
pub const TOAST_SHOW: &str = "toast:show";

/// Emitted once when a run transitions from `Pending` to `Running`.
///
/// Payload shape: `{ run_id: string, project_id: string }`.
/// Frontend counterpart: `RUN_STARTED` in `src/ipc/events.ts`.
pub const RUN_STARTED: &str = "run:started";

/// Emitted for each structured event parsed from the child's stdout.
///
/// Payload shape: `{ run_id: string, event: RunEvent }`.
/// Frontend counterpart: `RUN_EVENT` in `src/ipc/events.ts`.
pub const RUN_EVENT: &str = "run:event";

/// Emitted once when a run reaches a terminal state (Completed, Failed, or Stopped).
///
/// Payload shape: `{ run_id: string, status: RunStatus, exit_code: number | null }`.
/// Frontend counterpart: `RUN_FINISHED` in `src/ipc/events.ts`.
pub const RUN_FINISHED: &str = "run:finished";
