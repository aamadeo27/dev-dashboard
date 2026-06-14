// Typed wrappers for Tauri commands. See KB §5. Additional wrappers added per task.
import { invoke } from "@tauri-apps/api/core";
import type {
  CliCheck,
  GitStatus,
  LaunchInput,
  Project,
  Run,
  Sequence,
  Settings,
  SettingsPatch,
} from "./bindings";

export function logFrontendError(message: string, stack?: string, route?: string): void {
  invoke("log_frontend_error", { message, stack, route }).catch(() => {});
}

/** Fetch the current application settings from the backend store. */
export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/**
 * Apply a partial settings patch. Only non-null fields are merged.
 * Returns the full updated Settings on success.
 * Throws an AppError-shaped object on validation failure or I/O error.
 */
export function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return invoke<Settings>("update_settings", { patch });
}

/** Open the log folder in the OS file manager. */
export function openLogsFolder(): Promise<void> {
  return invoke<void>("open_logs_folder");
}

/**
 * Probe the Claude CLI binary.
 *
 * Pass an absolute path string to override the stored setting, or `undefined`
 * to let the backend resolve: stored `claude_cli_path` → `"claude"` on PATH.
 * Errors are soft — check `CliCheck.found` and `CliCheck.error` on the result.
 */
export function verifyClaudeCli(pathOverride?: string): Promise<CliCheck> {
  return invoke<CliCheck>("verify_claude_cli", {
    pathOverride: pathOverride ?? null,
  });
}

// ---------------------------------------------------------------------------
// Project commands (T2.1)
// Note: renameProject is intentionally NOT exposed here (internal Rust only,
// per KB §5.1). The Tauri command exists but has no TS wrapper.
// ---------------------------------------------------------------------------

/** Return all registered projects. `is_missing` is computed live by the backend. */
export function listProjects(): Promise<Project[]> {
  return invoke<Project[]>("list_projects");
}

/**
 * Register a new project at the given absolute path.
 *
 * The backend canonicalizes the path, rejects duplicates (throws
 * `AppError` with code `"ALREADY_EXISTS"`), and uses the basename as the
 * initial project name.
 */
export function addProject(path: string): Promise<Project> {
  return invoke<Project>("add_project", { path });
}

/**
 * Remove a registered project by id.
 *
 * Throws `AppError` with code `"NOT_FOUND"` if no project has that id.
 */
export function removeProject(id: string): Promise<void> {
  return invoke<void>("remove_project", { id });
}

/**
 * Update the filesystem path of a project.
 *
 * Throws `AppError` with code `"NOT_FOUND"` if the id is unknown, or
 * `"ALREADY_EXISTS"` if `newPath` is already registered to a different
 * project.
 */
export function relocateProject(id: string, newPath: string): Promise<Project> {
  return invoke<Project>("relocate_project", { id, newPath });
}

/**
 * Replace the tags of a project.
 *
 * Tags are lowercased, trimmed, and deduplicated by the backend before
 * saving. Throws `AppError` with code `"NOT_FOUND"` if the id is unknown.
 */
export function setProjectTags(id: string, tags: string[]): Promise<Project> {
  return invoke<Project>("set_project_tags", { id, tags });
}

/**
 * Re-scan a project's language and package manager.
 *
 * The backend re-runs marker-file detection on the project's stored path,
 * updates the fields, persists to disk, and returns the updated Project.
 * Throws `AppError` with code `"NOT_FOUND"` if no project has the given id.
 */
export function scanProject(id: string): Promise<Project> {
  return invoke<Project>("scan_project", { id });
}

// ---------------------------------------------------------------------------
// Git poller commands (T2.3)
// ---------------------------------------------------------------------------

/**
 * Returns the cached git status for a project, or `null` if the poller has
 * not yet completed a poll cycle for this project.
 *
 * The backend updates the cache every `git_poll_interval_secs` of unpaused
 * window time. `error` on the returned GitStatus indicates the last poll
 * failed (other fields may be stale).
 */
export function getGitStatus(id: string): Promise<GitStatus | null> {
  return invoke<GitStatus | null>("get_git_status", { id });
}

/**
 * Replaces the set of visible project IDs.
 *
 * The git poller only polls projects in this set, so call this whenever the
 * viewport changes (scroll, filter, tab switch). Passing an empty array
 * effectively pauses polling without clearing the cache.
 */
export function setVisibleProjects(ids: string[]): Promise<void> {
  return invoke<void>("set_visible_projects", { ids });
}

// ---------------------------------------------------------------------------
// Platform open commands (T2.8)
// ---------------------------------------------------------------------------

/**
 * Open the project directory in an editor.
 *
 * Uses `$EDITOR` if set; otherwise falls back to the OS default file
 * association via `tauri_plugin_opener`. Throws `AppError` with code
 * `"NOT_FOUND"` if no project has the given id, or `"IO"` if the OS open
 * fails. On IO failure the backend also emits a `toast:show` error event.
 */
export function openInEditor(id: string): Promise<void> {
  return invoke<void>("open_in_editor", { id });
}

/**
 * Open the project directory in the OS default terminal.
 *
 * Uses `tauri_plugin_opener` to trigger the OS default handler for the
 * directory. Throws `AppError` with code `"NOT_FOUND"` if no project has
 * the given id, or `"IO"` if the OS open fails. On IO failure the backend
 * also emits a `toast:show` error event.
 */
export function openInTerminal(id: string): Promise<void> {
  return invoke<void>("open_in_terminal", { id });
}

// ---------------------------------------------------------------------------
// Sequence commands (T3.1)
// ---------------------------------------------------------------------------

/**
 * Return all sequences for the given project.
 *
 * Reads `<project_path>/.claude/sequences/*.md`. Results are cached in
 * memory by the backend and invalidated when the directory mtime changes.
 * Throws `AppError` with code `"NOT_FOUND"` if no project has the given id.
 */
export function listSequences(projectId: string): Promise<Sequence[]> {
  return invoke<Sequence[]>("list_sequences", { projectId });
}

/**
 * Bust the sequence cache for a project and return the freshly scanned list.
 *
 * Forces a re-scan of `<project_path>/.claude/sequences/*.md` regardless of
 * the cached directory mtime.
 * Throws `AppError` with code `"NOT_FOUND"` if no project has the given id.
 */
export function refreshSequences(projectId: string): Promise<Sequence[]> {
  return invoke<Sequence[]>("refresh_sequences", { projectId });
}

// ---------------------------------------------------------------------------
// Run commands (T4.3 / T5.10)
// ---------------------------------------------------------------------------

/**
 * Return all runs for a project, newest first (meta.json only — no transcript).
 *
 * Throws `AppError` with code `"NOT_FOUND"` if no project has the given id.
 */
export function listRuns(projectId: string): Promise<Run[]> {
  return invoke<Run[]>("list_runs", { projectId });
}

/**
 * Launch a Claude CLI child process for the given project and sequence.
 *
 * Returns the initial `Run` record (status = "Pending") synchronously.
 * The backend then transitions to "Running" and streams events via
 * `run:started`, `run:event`, and `run:finished`.
 * Throws `AppError` with code `"NOT_FOUND"` if the project id is unknown.
 */
export function launchRun(input: LaunchInput): Promise<Run> {
  return invoke<Run>("launch_run", { input });
}

/**
 * Cancel an active run.
 *
 * The backend fires the cancellation token, kills the child process, and
 * emits `run:finished` with status `"Stopped"`.
 * Throws `AppError` with code `"NOT_FOUND"` if no active run has the given id.
 */
export function stopRun(runId: string): Promise<void> {
  return invoke<void>("stop_run", { runId });
}

/**
 * Write text (followed by a newline) to the child's stdin.
 *
 * Throws `AppError` with code `"NOT_FOUND"` if no active run has the given id,
 * or `"INVALID_INPUT"` if the run is no longer accepting input.
 */
export function sendInput(runId: string, text: string): Promise<void> {
  return invoke<void>("send_input", { runId, text });
}
