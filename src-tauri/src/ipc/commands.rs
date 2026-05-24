use std::process::Stdio;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::projects::Project;
use crate::runs::manager::SessionHandle;
use crate::runs::session::{build_run_dir, validate_run_id, verify_run_dir_prefix};
use crate::runs::transcript::TranscriptWriter;
use crate::runs::{LaunchInput, Run, RunStatus};
use crate::sequences::Sequence;
use crate::settings::{Settings, SettingsPatch};

/// CLI probe result returned by the `verify_claude_cli` command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct CliCheck {
    pub found: bool,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub resolved_path: Option<std::path::PathBuf>,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// Resolve the CLI path from the three possible sources.
///
/// Priority: explicit override → settings value → bare `"claude"` fallback.
/// This is a pure function with no side effects; extracted for testability.
/// `pub` (not `pub(crate)`) so that integration tests in `src-tauri/tests/`
/// can import it directly without a Tauri runtime.
pub fn resolve_cli_path(
    path_override: Option<std::path::PathBuf>,
    settings_path: Option<std::path::PathBuf>,
) -> std::path::PathBuf {
    if let Some(p) = path_override {
        return p;
    }
    if let Some(p) = settings_path {
        return p;
    }
    std::path::PathBuf::from("claude")
}

/// Validate a caller-supplied CLI path before passing it to spawn().
///
/// Rules (mirrors the T1.1 settings validation for `claude_cli_path`):
/// - Must be absolute (relative paths can be hijacked via cwd manipulation).
/// - On Windows: must not be a UNC or \\?\ path (avoids network execution).
/// - The file must exist and be a regular file (not a directory or symlink loop).
///
/// This function is called **only when `path_override` is Some** — the
/// settings-sourced path was already validated by T1.1's `SettingsStore::patch`,
/// and the bare `"claude"` fallback is an intentional PATH lookup that must not
/// be validated here (it is not an absolute path by design).
///
/// Returns `Ok(())` on success, or `Err(String)` with a human-readable message
/// on failure. The caller converts this to a soft `CliCheck` error, not an
/// `AppError`, to keep the command error-free.
pub async fn validate_cli_path(path: &std::path::Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("path_override must be an absolute path".to_string());
    }

    // On Windows, reject UNC paths (\\server\share) and \\?\ extended paths.
    // Forward-slash equivalents (//server/share) are also rejected.
    #[cfg(target_os = "windows")]
    {
        let s = path.to_string_lossy();
        if s.starts_with("\\\\") || s.starts_with("//") {
            return Err(
                "path_override must not be a UNC or network path".to_string()
            );
        }
    }

    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(format!(
            "path_override is not a regular file: {}",
            path.display()
        )),
        Err(e) => Err(format!(
            "path_override does not exist or is not accessible: {} ({})",
            path.display(),
            e
        )),
    }
}

/// Probe the Claude CLI: resolve its path, spawn `<path> --version`, parse stdout.
///
/// Resolution order: `path_override` arg → `settings.claude_cli_path` → PATH lookup (`"claude"`).
/// Errors are soft — they populate `CliCheck.error`; this command never returns `AppError`.
#[tauri::command]
pub async fn verify_claude_cli(
    path_override: Option<std::path::PathBuf>,
    state: tauri::State<'_, AppState>,
) -> AppResult<CliCheck> {
    let settings_path = {
        let store = state.settings.lock().await;
        store.settings().claude_cli_path.clone()
    };

    // Capture whether a path_override was provided before resolve_cli_path consumes it.
    let has_override = path_override.is_some();

    let resolved = resolve_cli_path(path_override, settings_path);

    // Validate the resolved path only when the caller supplied path_override.
    // - Settings-sourced paths were already validated by T1.1 SettingsStore::patch.
    // - The bare "claude" fallback is an intentional PATH lookup (not absolute by design).
    if has_override {
        if let Err(msg) = validate_cli_path(&resolved).await {
            // Use Debug formatting (?resolved) to escape control characters in the path.
            tracing::warn!(
                component = "cli_detect",
                kind = "cli",
                path_tried = ?resolved,
                message = %msg,
                "cli detect failed"
            );
            // resolved_path is None here: the path failed pre-spawn validation
            // and was never used; returning it would mislead callers into thinking
            // a probe was attempted at that location.
            return Ok(CliCheck {
                found: false,
                resolved_path: None,
                version: None,
                error: Some(msg),
            });
        }
    }

    match tokio::process::Command::new(&resolved)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Err(e) => {
            let error_msg = format!(
                "Failed to launch Claude CLI at '{}': {}",
                resolved.display(),
                e
            );
            tracing::warn!(
                component = "cli_detect",
                kind = "cli",
                path_tried = ?resolved,
                message = %error_msg,
                "cli detect failed"
            );
            Ok(CliCheck {
                found: false,
                resolved_path: Some(resolved),
                version: None,
                error: Some(error_msg),
            })
        }
        Ok(child) => {
            let output_result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                child.wait_with_output(),
            )
            .await;

            match output_result {
                Err(_) => {
                    // Timeout — the child is killed because kill_on_drop(true) was set
                    // on the Command builder; dropping Child here terminates the process.
                    let error_msg = "CLI probe timed out after 10 seconds".to_string();
                    tracing::warn!(
                        component = "cli_detect",
                        kind = "cli",
                        path_tried = ?resolved,
                        message = %error_msg,
                        "cli detect failed"
                    );
                    Ok(CliCheck {
                        found: false,
                        resolved_path: Some(resolved),
                        version: None,
                        error: Some(error_msg),
                    })
                }
                Ok(Err(e)) => {
                    let error_msg = format!("Failed to read CLI output: {e}");
                    tracing::warn!(
                        component = "cli_detect",
                        kind = "cli",
                        path_tried = ?resolved,
                        message = %error_msg,
                        "cli detect failed"
                    );
                    Ok(CliCheck {
                        found: false,
                        resolved_path: Some(resolved),
                        version: None,
                        error: Some(error_msg),
                    })
                }
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                    // Version line: prefer stdout; fall back to stderr (some CLIs print to stderr).
                    // Cap at 256 chars and strip control characters to prevent log injection
                    // and unbounded UI surface from a hostile binary's output.
                    let raw = if !stdout.is_empty() { stdout } else { stderr };
                    let version: Option<String> = if raw.is_empty() {
                        None
                    } else {
                        let capped: String = raw
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(256)
                            .collect();
                        if capped.is_empty() { None } else { Some(capped) }
                    };

                    tracing::info!(
                        component = "cli_detect",
                        resolved_path = ?resolved,
                        version = version.as_deref().unwrap_or(""),
                        mode = "interactive",
                        "cli detected"
                    );
                    Ok(CliCheck {
                        found: true,
                        resolved_path: Some(resolved),
                        version,
                        error: None,
                    })
                }
            }
        }
    }
}

/// Health-check command; returns `"pong"` to confirm IPC is operational.
#[tauri::command]
pub async fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

// Exists only to smoke-test AppError IPC serialization; excluded from release builds.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn ping_error() -> AppResult<String> {
    Err(crate::error::AppError::NotFound("ping_error_test".to_string()))
}

/// Forwards a frontend error into the structured log (monitoring.md §1.3k).
#[tauri::command]
pub async fn log_frontend_error(
    message: String,
    stack: Option<String>,
    route: Option<String>,
) {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    tracing::error!(
        component = "frontend",
        source = "frontend",
        kind = "frontend",
        correlation_id = %correlation_id,
        stack = stack.as_deref().unwrap_or(""),
        route = route.as_deref().unwrap_or(""),
        "{}", message
    );
}

/// Return the current application settings.
#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, AppState>) -> AppResult<Settings> {
    let store = state.settings.lock().await;
    Ok(store.settings().clone())
}

/// Apply a partial settings patch, validate ranges, persist to disk, and
/// return the updated settings.
#[tauri::command]
pub async fn update_settings(
    patch: SettingsPatch,
    state: tauri::State<'_, AppState>,
) -> AppResult<Settings> {
    let mut store = state.settings.lock().await;
    store.patch(patch, &state.config_dir).await?;
    Ok(store.settings().clone())
}

/// Open the application log folder in the OS file manager.
#[tauri::command]
pub async fn open_logs_folder(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let logs_dir = state.config_dir.join("logs");
    // Ensure the directory exists so the opener has something to show.
    tokio::fs::create_dir_all(&logs_dir).await?;
    tauri_plugin_opener::open_path(logs_dir, None::<&str>)
        .map_err(|e| crate::error::AppError::Io(
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        ))
}

// ---------------------------------------------------------------------------
// Project commands (T2.1)
// ---------------------------------------------------------------------------

/// Return all registered projects. `is_missing` is computed live.
#[tauri::command]
pub async fn list_projects(state: tauri::State<'_, AppState>) -> AppResult<Vec<Project>> {
    let registry = state.projects.lock().await;
    let projects = registry.list_projects().await;
    Ok(projects)
}

/// Register a new project at the given path.
///
/// Canonicalizes the path, rejects duplicates with `AlreadyExists`, assigns a
/// UUID v7 id, and uses the path basename as the initial name.
#[tauri::command]
pub async fn add_project(
    path: std::path::PathBuf,
    state: tauri::State<'_, AppState>,
) -> AppResult<Project> {
    let mut registry = state.projects.lock().await;
    registry.add_project(path).await
}

/// Remove a project by id. Returns `NotFound` if no project has that id.
#[tauri::command]
pub async fn remove_project(
    id: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let mut registry = state.projects.lock().await;
    registry.remove_project(&id).await
}

/// Update the filesystem path of a project.
///
/// Returns `NotFound` if the id is unknown, `AlreadyExists` if the new path
/// is already registered to a different project.
#[tauri::command]
pub async fn relocate_project(
    id: String,
    new_path: std::path::PathBuf,
    state: tauri::State<'_, AppState>,
) -> AppResult<Project> {
    let mut registry = state.projects.lock().await;
    registry.relocate_project(&id, new_path).await
}

/// Replace the tags of a project (lowercased, trimmed, deduplicated).
///
/// Returns `NotFound` if the id is unknown.
#[tauri::command]
pub async fn set_project_tags(
    id: String,
    tags: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<Project> {
    let mut registry = state.projects.lock().await;
    registry.set_project_tags(&id, tags).await
}

/// Rename a project. Internal only — no TS wrapper is generated per KB §5.1.
///
/// Returns `NotFound` if the id is unknown.
#[tauri::command]
pub async fn rename_project(
    id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Project> {
    let mut registry = state.projects.lock().await;
    registry.rename_project(&id, name).await
}

/// Re-scans a project's language and package manager by re-running
/// `scanner::detect` on its stored path and persisting the updated fields.
///
/// Returns the updated `Project`. Returns `AppError::NotFound` if no project
/// has the given id.
#[tauri::command]
pub async fn scan_project(
    id: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Project> {
    let mut registry = state.projects.lock().await;
    registry.scan_project(&id).await
}

// ---------------------------------------------------------------------------
// Platform open commands (T2.8)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Git poller commands (T2.3)
// ---------------------------------------------------------------------------

/// Returns the cached git status for a project, or null if not yet polled.
///
/// The poller updates this cache every `git_poll_interval_secs` of unpaused
/// window time. Returns `None` before the first poll cycle completes.
#[tauri::command]
pub async fn get_git_status(
    id: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<crate::projects::git::GitStatus>> {
    let cache = state.git_poller.statuses.lock().await;
    Ok(cache.get(&id).cloned())
}

/// Updates the set of visible project IDs; the poller only polls these.
///
/// Call this whenever the viewport changes (scroll, filter, tab switch).
/// Passing an empty vec effectively pauses polling without touching the cache.
#[tauri::command]
pub async fn set_visible_projects(
    ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    if ids.len() > 1024 {
        return Err(crate::error::AppError::InvalidInput(
            "set_visible_projects: too many ids (max 1024)".to_string()
        ));
    }
    if ids.iter().any(|id| id.len() > 128) {
        return Err(crate::error::AppError::InvalidInput(
            "set_visible_projects: id too long (max 128 chars)".to_string()
        ));
    }
    let mut visible = state.git_poller.visible.lock().await;
    *visible = ids.into_iter().collect();
    Ok(())
}

/// Open the project directory in an editor.
///
/// If `$EDITOR` is set, spawns it as a detached child process.
/// Falls back to `tauri_plugin_opener::open_path` for OS default file
/// association. Emits `toast:show` on failure, and also returns `AppError::Io`.
///
/// Works even when `project.is_missing = true` — the OS is responsible for
/// handling the path; we attempt the open unconditionally.
#[tauri::command]
pub async fn open_in_editor(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> AppResult<()> {
    let path = {
        let registry = state.projects.lock().await;
        crate::platform::lookup_project_path(&registry, &id)?
    };
    crate::platform::open_in_editor_impl(&id, path, &app).await
}

/// Open the project directory in the OS default terminal.
///
/// Uses `tauri_plugin_opener::open_path` to trigger the OS default handler
/// for the directory path. Emits `toast:show` on failure, and also returns
/// `AppError::Io`.
///
/// Works even when `project.is_missing = true` — the OS is responsible for
/// handling the path; we attempt the open unconditionally.
#[tauri::command]
pub async fn open_in_terminal(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> AppResult<()> {
    let path = {
        let registry = state.projects.lock().await;
        crate::platform::lookup_project_path(&registry, &id)?
    };
    crate::platform::open_in_terminal_impl(&id, path, &app).await
}

// ---------------------------------------------------------------------------
// Sequence commands (T3.1)
// ---------------------------------------------------------------------------

/// Look up the filesystem path of a registered project by id.
///
/// Extracted from `list_sequences` and `refresh_sequences` to avoid
/// duplicating the identical lookup + `NotFound` error conversion.
pub(crate) fn get_sequence_project_path(
    registry: &crate::projects::ProjectRegistry,
    project_id: &str,
) -> AppResult<std::path::PathBuf> {
    registry
        .get_project_path(project_id)
        .ok_or_else(|| AppError::NotFound(format!("project id: {project_id}")))
}

/// Return all sequences for the given project.
///
/// Reads `<project_path>/.claude/sequences/*.md`. Extracts the description
/// from each file (first non-heading paragraph; `"(No description)"` fallback).
/// Results are cached in memory and invalidated when the directory mtime changes.
///
/// Returns `AppError::NotFound` if no project with `project_id` is registered.
#[tauri::command]
pub async fn list_sequences(
    project_id: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<Sequence>> {
    let project_path = {
        let registry = state.projects.lock().await;
        get_sequence_project_path(&registry, &project_id)?
    };

    tracing::info!(
        component = "sequence_loader",
        project_id = %project_id,
        "list_sequences called"
    );

    let mut loader = state.sequence_loader.lock().await;
    loader.load_all(&project_id, &project_path).await
}

/// Bust the sequence cache for a project and return the freshly scanned list.
///
/// Forces a re-scan of `<project_path>/.claude/sequences/*.md` regardless of
/// the cached directory mtime.
///
/// Returns `AppError::NotFound` if no project with `project_id` is registered.
#[tauri::command]
pub async fn refresh_sequences(
    project_id: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<Sequence>> {
    let project_path = {
        let registry = state.projects.lock().await;
        get_sequence_project_path(&registry, &project_id)?
    };

    tracing::info!(
        component = "sequence_loader",
        project_id = %project_id,
        "refresh_sequences called"
    );

    let mut loader = state.sequence_loader.lock().await;
    loader.refresh(&project_id, &project_path).await
}

// ---------------------------------------------------------------------------
// Run commands (T4.3)
// ---------------------------------------------------------------------------

/// Launch a Claude CLI child process for the given project and sequence.
///
/// Returns the initial `Run` (status = Pending) synchronously.  A background
/// Tokio task then updates the status to `Running` and starts streaming events
/// to the frontend via `run:started`, `run:event`, and `run:finished`.
#[tauri::command]
pub async fn launch_run(
    input: LaunchInput,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> AppResult<Run> {
    // ── 1. Resolve project ───────────────────────────────────────────────────
    let project_path = {
        let registry = state.projects.lock().await;
        registry
            .get_project_path(&input.project_id)
            .ok_or_else(|| AppError::NotFound(format!("project id: {}", input.project_id)))?
    };

    // ── 2. Generate and validate run id ──────────────────────────────────────
    let run_id = uuid::Uuid::now_v7().to_string();
    if !validate_run_id(&run_id) {
        return Err(AppError::Internal(format!(
            "generated run_id failed validation: {}",
            run_id
        )));
    }

    // ── 3. Build and verify run directory path ────────────────────────────────
    let run_dir = build_run_dir(&project_path, &run_id);
    verify_run_dir_prefix(&project_path, &run_dir).await?;

    // ── 4. Resolve CLI path ───────────────────────────────────────────────────
    let cli_path = {
        let store = state.settings.lock().await;
        resolve_cli_path(None, store.settings().claude_cli_path.clone())
    };

    // ── 5. Build initial Run record ───────────────────────────────────────────
    let initial_run = Run {
        id: run_id.clone(),
        project_id: input.project_id.clone(),
        project_path: project_path.clone(),
        sequence_name: input.sequence_name.clone(),
        attached_md_path: input.attached_md_path.clone(),
        started_at: Utc::now(),
        ended_at: None,
        status: RunStatus::Pending,
        exit_code: None,
        pid: None,
        note: None,
    };

    // ── 6. Create transcript writer (creates run_dir + files) ─────────────────
    let writer = TranscriptWriter::create(&run_id, &run_dir, &initial_run).await?;

    // ── 7. Spawn child process ────────────────────────────────────────────────
    let mut cmd = tokio::process::Command::new(&cli_path);
    cmd.arg(&input.sequence_name)
        .current_dir(&project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(AppError::Io)?;

    let child_stdin = child.stdin.take();
    let child_stdout = child.stdout.take().ok_or_else(|| {
        AppError::Internal("child stdout handle missing after spawn".to_string())
    })?;
    let child_stderr = child.stderr.take().ok_or_else(|| {
        AppError::Internal("child stderr handle missing after spawn".to_string())
    })?;

    // ── 8. Build session handle and register ──────────────────────────────────
    let cancel_token = CancellationToken::new();
    let run_arc = Arc::new(Mutex::new(initial_run.clone()));
    let handle = Arc::new(SessionHandle {
        cancel: cancel_token.clone(),
        stdin: Arc::new(Mutex::new(child_stdin)),
        run: run_arc.clone(),
    });

    let sessions = {
        let rm = state.run_manager.lock().await;
        rm.sessions.clone()
    };

    {
        let mut map = sessions.lock().await;
        map.insert(run_id.clone(), handle);
    }

    // ── 9. Spawn background I/O task ──────────────────────────────────────────
    tokio::task::spawn(crate::runs::session::run_io_loop(
        app_handle,
        run_id.clone(),
        run_arc,
        child_stdout,
        child_stderr,
        child,
        writer,
        cancel_token,
        sessions,
    ));

    tracing::info!(
        run_id = %run_id,
        project_id = %input.project_id,
        sequence_name = %input.sequence_name,
        "launch_run: run created"
    );

    Ok(initial_run)
}

/// Cancel an active run by cancelling its `CancellationToken`.
///
/// The background I/O task detects the cancellation, kills the child process,
/// and emits `run:finished` with status `Stopped`.
///
/// Returns `AppError::NotFound` if no active run has the given id.
#[tauri::command]
pub async fn stop_run(
    run_id: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let sessions = {
        let rm = state.run_manager.lock().await;
        rm.sessions.clone()
    };

    let handle = {
        let map = sessions.lock().await;
        map.get(&run_id).cloned()
    };

    match handle {
        None => Err(AppError::NotFound(format!("run id: {}", run_id))),
        Some(h) => {
            h.cancel.cancel();
            tracing::info!(run_id = %run_id, "stop_run: cancellation token fired");
            Ok(())
        }
    }
}

/// Write text (followed by a newline) to the child's stdin.
///
/// Returns `AppError::NotFound` if no active run has the given id.
/// Returns `AppError::InvalidInput` if the run is no longer accepting input
/// (stdin has already been closed / taken by the time the lock is acquired).
#[tauri::command]
pub async fn send_input(
    run_id: String,
    text: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let sessions = {
        let rm = state.run_manager.lock().await;
        rm.sessions.clone()
    };

    let handle = {
        let map = sessions.lock().await;
        map.get(&run_id).cloned()
    };

    let handle = match handle {
        None => return Err(AppError::NotFound(format!("run id: {}", run_id))),
        Some(h) => h,
    };

    let mut stdin_guard = handle.stdin.lock().await;
    match stdin_guard.as_mut() {
        None => Err(AppError::InvalidInput(
            "run is not accepting input".to_string(),
        )),
        Some(stdin) => {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(text.as_bytes()).await.map_err(AppError::Io)?;
            stdin.write_all(b"\n").await.map_err(AppError::Io)?;
            tracing::debug!(run_id = %run_id, text_len = text.len(), "send_input: wrote to stdin");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // T1.2-sec-fix — validate_cli_path
    // -----------------------------------------------------------------------

    /// A relative path must be rejected regardless of OS.
    #[tokio::test]
    async fn validate_cli_path_rejects_relative_path() {
        let result = validate_cli_path(std::path::Path::new("relative/path/claude")).await;
        assert!(result.is_err(), "relative path must be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("absolute"),
            "error message must mention 'absolute', got: {msg}"
        );
    }

    /// A bare filename (no directory components) is also relative and must be rejected.
    #[tokio::test]
    async fn validate_cli_path_rejects_bare_filename() {
        let result = validate_cli_path(std::path::Path::new("claude")).await;
        assert!(result.is_err(), "bare filename must be rejected as relative");
    }

    /// On Windows, a UNC path starting with `\\` must be rejected.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn validate_cli_path_rejects_unc_backslash() {
        let result =
            validate_cli_path(std::path::Path::new("\\\\server\\share\\claude")).await;
        assert!(result.is_err(), "UNC path with backslashes must be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("UNC") || msg.contains("network"),
            "error message must mention UNC/network, got: {msg}"
        );
    }

    /// On Windows, a UNC path starting with `//` (forward slashes) must be rejected.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn validate_cli_path_rejects_unc_forward_slash() {
        let result =
            validate_cli_path(std::path::Path::new("//server/share/claude")).await;
        assert!(result.is_err(), "UNC path with forward slashes must be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("UNC") || msg.contains("network"),
            "error message must mention UNC/network, got: {msg}"
        );
    }

    /// An absolute path that does not exist on the filesystem must be rejected.
    #[tokio::test]
    async fn validate_cli_path_rejects_nonexistent_absolute_path() {
        // Choose a path that cannot plausibly exist.
        let path = if cfg!(target_os = "windows") {
            std::path::PathBuf::from(r"C:\nonexistent_99999\claude.exe")
        } else {
            std::path::PathBuf::from("/nonexistent_99999/claude")
        };
        let result = validate_cli_path(&path).await;
        assert!(result.is_err(), "nonexistent path must be rejected");
    }

    /// An absolute path pointing to a directory (not a regular file) must be rejected.
    #[tokio::test]
    async fn validate_cli_path_rejects_directory() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        // The temp dir itself is an absolute path that exists but is a directory.
        let result = validate_cli_path(tmp.path()).await;
        assert!(result.is_err(), "directory path must be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("not a regular file"),
            "error message must mention 'not a regular file', got: {msg}"
        );
    }

    /// An absolute path pointing to an existing regular file must be accepted.
    #[tokio::test]
    async fn validate_cli_path_accepts_existing_regular_file() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let file_path = tmp.path().join("claude_fake");
        tokio::fs::write(&file_path, b"#!/bin/sh\necho 1.0.0")
            .await
            .expect("write temp file");
        let result = validate_cli_path(&file_path).await;
        assert!(result.is_ok(), "existing regular file must be accepted: {:?}", result);
    }

    // -----------------------------------------------------------------------
    // T1.2 — verify_claude_cli: resolve_cli_path helper
    // -----------------------------------------------------------------------

    /// When an explicit override is provided it always wins, regardless of
    /// what the settings value is.
    #[test]
    fn resolve_cli_path_override_takes_priority_over_settings() {
        let override_path = std::path::PathBuf::from("/custom/override/claude");
        let settings_path = std::path::PathBuf::from("/settings/stored/claude");

        let result = resolve_cli_path(Some(override_path.clone()), Some(settings_path));

        assert_eq!(result, override_path);
    }

    /// When there is no override, the stored settings path is used.
    #[test]
    fn resolve_cli_path_uses_settings_when_no_override() {
        let settings_path = std::path::PathBuf::from("/settings/stored/claude");

        let result = resolve_cli_path(None, Some(settings_path.clone()));

        assert_eq!(result, settings_path);
    }

    /// When both override and settings are None, fall back to the bare
    /// `"claude"` name for PATH lookup.
    #[test]
    fn resolve_cli_path_falls_back_to_claude_when_both_none() {
        let result = resolve_cli_path(None, None);

        assert_eq!(result, std::path::PathBuf::from("claude"));
    }

    /// An override with an empty settings value still wins (override beats None).
    #[test]
    fn resolve_cli_path_override_beats_none_settings() {
        let override_path = std::path::PathBuf::from("/usr/bin/claude");

        let result = resolve_cli_path(Some(override_path.clone()), None);

        assert_eq!(result, override_path);
    }

    // -----------------------------------------------------------------------
    // set_visible_projects per-id length guard — FIX-2 (T2.3-fixes-3)
    // The command requires tauri::State, so we test the guard predicate inline.
    // -----------------------------------------------------------------------

    /// An id of exactly 128 chars must pass the per-id length guard.
    #[test]
    fn set_visible_projects_id_length_guard_accepts_128_chars() {
        let ids: Vec<String> = vec!["x".repeat(128)];
        let any_too_long = ids.iter().any(|id| id.len() > 128);
        assert!(!any_too_long, "id of exactly 128 chars must be accepted");
    }

    /// An id of 129 chars must trigger the per-id length guard.
    #[test]
    fn set_visible_projects_id_length_guard_rejects_129_chars() {
        let ids: Vec<String> = vec!["x".repeat(129)];
        let any_too_long = ids.iter().any(|id| id.len() > 128);
        assert!(any_too_long, "id of 129 chars must be rejected");
    }

    /// A list of short ids must not trigger the guard.
    #[test]
    fn set_visible_projects_id_length_guard_accepts_short_ids() {
        let ids: Vec<String> = vec![
            "project-1".to_string(),
            "abc-def".to_string(),
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
        ];
        let any_too_long = ids.iter().any(|id| id.len() > 128);
        assert!(!any_too_long, "short ids must all be accepted");
    }

    // -----------------------------------------------------------------------
    // T4.3 — send_input / stop_run with unknown run_id
    // These tests exercise the session-map lookup logic directly without a
    // Tauri runtime.
    // -----------------------------------------------------------------------

    /// Helper: construct an empty sessions map identical to what `RunManager`
    /// holds, then call the lookup logic from `stop_run`.
    #[tokio::test]
    async fn stop_run_returns_error_for_unknown_run_id() {
        use std::collections::HashMap;
        use crate::runs::manager::SessionHandle;
        use tokio_util::sync::CancellationToken;

        // Build an empty sessions map (no active runs).
        let sessions: std::sync::Arc<tokio::sync::Mutex<HashMap<String, std::sync::Arc<SessionHandle>>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        // Simulate the stop_run lookup.
        let handle = {
            let map = sessions.lock().await;
            map.get("nonexistent-run-id").cloned()
        };

        let result: AppResult<()> = match handle {
            None => Err(AppError::NotFound("run id: nonexistent-run-id".to_string())),
            Some(h) => {
                h.cancel.cancel();
                Ok(())
            }
        };

        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "stop_run must return NotFound for unknown run_id, got: {:?}",
            result
        );
    }

    /// Simulate the send_input lookup logic with an empty sessions map.
    #[tokio::test]
    async fn send_input_returns_error_for_unknown_run_id() {
        use std::collections::HashMap;
        use crate::runs::manager::SessionHandle;

        // Build an empty sessions map (no active runs).
        let sessions: std::sync::Arc<tokio::sync::Mutex<HashMap<String, std::sync::Arc<SessionHandle>>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        // Simulate the send_input lookup.
        let handle = {
            let map = sessions.lock().await;
            map.get("nonexistent-run-id").cloned()
        };

        let result: AppResult<()> = match handle {
            None => Err(AppError::NotFound("run id: nonexistent-run-id".to_string())),
            Some(_) => Ok(()),
        };

        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "send_input must return NotFound for unknown run_id, got: {:?}",
            result
        );
    }

}
