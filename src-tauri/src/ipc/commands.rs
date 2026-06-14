use std::process::Stdio;
use std::sync::Arc;

use chrono::Utc;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::projects::Project;
use crate::runs::manager::SessionHandle;
use crate::runs::session::{build_run_dir, validate_run_id, verify_run_dir_prefix, RunIoContext};
use crate::runs::transcript::TranscriptWriter;
use crate::runs::{LaunchInput, Run, RunStatus, StepFailureChoice};
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
            return Err("path_override must not be a UNC or network path".to_string());
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
            let output_result =
                tokio::time::timeout(std::time::Duration::from_secs(10), child.wait_with_output())
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
                        let capped: String =
                            raw.chars().filter(|c| !c.is_control()).take(256).collect();
                        if capped.is_empty() {
                            None
                        } else {
                            Some(capped)
                        }
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
    Err(crate::error::AppError::NotFound(
        "ping_error_test".to_string(),
    ))
}

/// Forwards a frontend error into the structured log (monitoring.md §1.3k).
#[tauri::command]
pub async fn log_frontend_error(message: String, stack: Option<String>, route: Option<String>) {
    // Sanitize fields: cap length and strip control chars (< 0x20 except \t, plus 0x7F).
    // `message` also strips \n to prevent log-line forging in line-oriented log sinks.
    // `stack` keeps \n so multi-line stack traces remain readable.
    let sanitize = |s: &str, max_len: usize, keep_newline: bool| -> String {
        s.chars()
            .filter(|&c| (keep_newline && c == '\n') || c == '\t' || (c >= '\x20' && c != '\x7F'))
            .take(max_len)
            .collect()
    };

    let message = sanitize(&message, 4096, false);
    let stack = stack.as_deref().map(|s| sanitize(s, 8192, true));
    let route = route.as_deref().map(|s| sanitize(s, 512, false));

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
        .map_err(|e| crate::error::AppError::Io(std::io::Error::other(e.to_string())))
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
pub async fn remove_project(id: String, state: tauri::State<'_, AppState>) -> AppResult<()> {
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
pub async fn scan_project(id: String, state: tauri::State<'_, AppState>) -> AppResult<Project> {
    let mut registry = state.projects.lock().await;
    registry.scan_project(&id).await
}

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
            "set_visible_projects: too many ids (max 1024)".to_string(),
        ));
    }
    if ids.iter().any(|id| id.len() > 128) {
        return Err(crate::error::AppError::InvalidInput(
            "set_visible_projects: id too long (max 128 chars)".to_string(),
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

/// Returns `true` iff `name` is a safe sequence name that can be passed to the
/// CLI without risk of flag injection or shell special-character issues.
///
/// Rules:
/// - Must not be empty.
/// - Must not be longer than 256 characters.
/// - Must not start with `-` (prevents `--print`, `--help`, etc.).
/// - Must not start with `/` (rejects absolute paths).
/// - No path component (split on `/`) may equal `..` (rejects traversal like
///   `../../etc/passwd` or `../sibling`).
/// - Every character must be in `[A-Za-z0-9._\-/ ]` (alphanumeric, dot,
///   underscore, hyphen, forward-slash, and space are the only allowed chars).
pub fn is_valid_sequence_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.len() > 256 {
        return false;
    }
    if name.starts_with('-') {
        return false;
    }
    if name.starts_with('/') {
        return false;
    }
    // Reject `..`, `.`, and empty segments (produced by leading/trailing/consecutive `/`).
    if name
        .split('/')
        .any(|seg| seg == ".." || seg == "." || seg.is_empty())
    {
        return false;
    }
    name.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '/' || c == ' '
    })
}

/// Read the contents of an attached Markdown file, enforcing a 1 MiB size cap.
///
/// Returns `AppError::NotFound` if the path does not exist or cannot be read,
/// so that `launch_run` can abort before spawning the child process.
/// Returns `AppError::InvalidInput` if the file exceeds `MAX_ATTACHED_MD_BYTES`.
///
/// The 1 MiB cap is a safety bound: the Claude CLI receives this content via
/// stdin before the sequence prompt, and an unbounded file could exhaust memory
/// or cause the child to stall on a massive write.
pub async fn read_attached_md(path: &std::path::Path) -> AppResult<Vec<u8>> {
    const MAX_ATTACHED_MD_BYTES: u64 = 1_048_576; // 1 MiB

    // Probe existence and size before reading — gives a precise error message.
    let meta = tokio::fs::metadata(path).await.map_err(|e| {
        AppError::NotFound(format!(
            "attached_md_path '{}' not found or not accessible: {}",
            path.display(),
            e
        ))
    })?;

    if meta.len() > MAX_ATTACHED_MD_BYTES {
        return Err(AppError::InvalidInput(format!(
            "attached_md_path '{}' is too large ({} bytes, max {} bytes)",
            path.display(),
            meta.len(),
            MAX_ATTACHED_MD_BYTES
        )));
    }

    tokio::fs::read(path).await.map_err(|e| {
        AppError::NotFound(format!(
            "attached_md_path '{}' could not be read: {}",
            path.display(),
            e
        ))
    })
}

/// Launch a Claude CLI child process for the given project and sequence.
///
/// Returns the initial `Run` (status = Pending) synchronously.  A background
/// Tokio task then updates the status to `Running` and starts streaming events
/// to the frontend via `run:started`, `run:event`, and `run:finished`.
///
/// If `input.attached_md_path` is set, its contents are prepended to the
/// first stdin write (before any sequence prompt) and the path is recorded
/// in `meta.json` as part of the `Run` record.
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

    // ── 1b. Validate sequence_name ────────────────────────────────────────────
    if !is_valid_sequence_name(&input.sequence_name) {
        return Err(AppError::InvalidInput(format!(
            "sequence_name: invalid value {:?}. Must be non-empty, must not start with '-', \
             and may only contain [A-Za-z0-9._\\-/ ]",
            input.sequence_name
        )));
    }

    // ── 1c. Read attached_md_path eagerly (before any side effects) ────────────
    // Fail with NotFound before spawning the child or creating the run directory,
    // so the caller sees a clean error with no partial state written to disk.
    let attached_md_content: Option<Vec<u8>> = if let Some(ref md_path) = input.attached_md_path {
        tracing::info!(
            project_id = %input.project_id,
            attached_md_path = ?md_path,
            "launch_run: reading attached_md_path"
        );
        let content = read_attached_md(md_path).await?;
        Some(content)
    } else {
        None
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
        exit_note: None,
        retry_of: None,
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

    let mut child_stdin = child.stdin.take();
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("child stdout handle missing after spawn".to_string()))?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal("child stderr handle missing after spawn".to_string()))?;

    // ── 7b. Write attached_md content to child stdin (first write) ─────────────
    // The content was read and validated before spawn (step 1c).  Writing it here,
    // before the session handle is built, ensures the child receives the context
    // before any subsequent `send_input` calls.  A trailing newline separates the
    // context from the sequence prompt that follows.
    // Empty content is skipped — writing only a lone newline may confuse the CLI.
    if let Some(ref content) = attached_md_content {
        if !content.is_empty() {
            if let Some(ref mut stdin) = child_stdin {
                if let Err(e) = stdin.write_all(content).await {
                    tracing::warn!(
                        run_id = %run_id,
                        error = %e,
                        "launch_run: failed to write attached_md to stdin; continuing"
                    );
                } else if let Err(e) = stdin.write_all(b"\n").await {
                    tracing::warn!(
                        run_id = %run_id,
                        error = %e,
                        "launch_run: failed to write attached_md newline to stdin; continuing"
                    );
                } else {
                    tracing::info!(
                        run_id = %run_id,
                        bytes = content.len(),
                        "launch_run: wrote attached_md to stdin"
                    );
                }
            }
        }
    }

    // ── 8. Build session handle and register ──────────────────────────────────
    let cancel_token = CancellationToken::new();
    let run_arc = Arc::new(Mutex::new(initial_run.clone()));
    let (input_tx, input_rx) = tokio::sync::mpsc::channel::<String>(32);
    let (sf_tx, sf_rx) = tokio::sync::mpsc::channel::<StepFailureChoice>(2);
    let stdin_arc = Arc::new(Mutex::new(child_stdin));
    let handle = Arc::new(SessionHandle {
        cancel: cancel_token.clone(),
        stdin: stdin_arc.clone(),
        run: run_arc.clone(),
        input_tx,
        step_failure_tx: sf_tx,
    });

    let sessions = state.run_manager.lock().await.sessions_arc();

    // ── 9. Insert handle into sessions map, then spawn background I/O task ───
    // Insert BEFORE spawn so that run_io_loop's map.remove (Step 8) can never
    // fire before the insert.  If the child exits instantly, run_io_loop's
    // select! arm fires after this insert, finds and removes the key cleanly.
    // Spawning after insert is safe because CancellationToken is already
    // cancelled by the time stop_run could have fired it; the I/O task's first
    // select! will detect the cancelled token immediately and kill the child.
    {
        let mut map = sessions.lock().await;
        map.insert(run_id.clone(), handle);
    }

    // Instrument the spawned future at the call site so the
    // async-aware span is entered/exited correctly across await points.
    let span = tracing::info_span!("run_session", run_id = %run_id);
    let ctx = RunIoContext {
        run: run_arc,
        stdout: child_stdout,
        stderr: child_stderr,
        child,
        writer,
        cancel: cancel_token,
        sessions: sessions.clone(),
        input_rx,
        launch_input: input.clone(),
        cli_path: cli_path.clone(),
        stdin_arc,
        step_failure_rx: sf_rx,
        attached_md_content: attached_md_content.clone(),
    };
    // Detached background task; the JoinHandle is intentionally dropped.
    tokio::task::spawn(
        crate::runs::session::run_io_loop(app_handle, run_id.clone(), ctx).instrument(span),
    );

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
pub async fn stop_run(run_id: String, state: tauri::State<'_, AppState>) -> AppResult<()> {
    let sessions = state.run_manager.lock().await.sessions_arc();

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
    let sessions = state.run_manager.lock().await.sessions_arc();

    let handle = {
        let map = sessions.lock().await;
        map.get(&run_id).cloned()
    };

    let handle = match handle {
        None => return Err(AppError::NotFound(format!("run id: {}", run_id))),
        Some(h) => h,
    };

    // Cap input size to prevent unbounded writes to child stdin.
    const MAX_INPUT_BYTES: usize = 65_536; // 64 KiB
    if text.len() > MAX_INPUT_BYTES {
        return Err(AppError::InvalidInput(format!(
            "send_input: text too large ({} bytes, max {})",
            text.len(),
            MAX_INPUT_BYTES
        )));
    }

    // Write to child stdin first (borrow text).  If stdin write fails, the
    // UserInput channel send is skipped — we do not record input that was not
    // actually delivered.
    {
        let mut stdin_guard = handle.stdin.lock().await;
        match stdin_guard.as_mut() {
            None => {
                return Err(AppError::InvalidInput(
                    "run is not accepting input".to_string(),
                ));
            }
            Some(stdin) => {
                stdin
                    .write_all(text.as_bytes())
                    .await
                    .map_err(AppError::Io)?;
                stdin.write_all(b"\n").await.map_err(AppError::Io)?;
                tracing::debug!(run_id = %run_id, text_len = text.len(), "send_input: wrote to stdin");
            }
        }
    }

    // Move text into the UserInput channel for transcript recording.
    handle.input_tx.try_send(text).map_err(|e| match e {
        TrySendError::Full(_) => {
            AppError::InvalidInput("run input queue full — try again shortly".to_string())
        }
        TrySendError::Closed(_) => AppError::InvalidInput("run is not accepting input".to_string()),
    })
}

/// Respond to a step-failure prompt for an active run.
///
/// Sends `choice` to the background I/O loop which is waiting (up to 60 s) for
/// a response after detecting a `StepFailed` event in the child's stdout.
///
/// | Choice   | Action in I/O loop                                               |
/// |----------|------------------------------------------------------------------|
/// | Continue | Write `"\n"` to child stdin; if no new output in 2 s → kill + re-invoke |
/// | Retry    | Kill child, re-invoke with identical `LaunchInput`; `retry_of` set in new run's `meta.json` |
/// | Skip     | Kill child, re-invoke with `"Skip the previous failing step and continue. " + original prompt` |
/// | Abort    | Kill child, mark run `RunStatus::Failed` with `exit_note = "Aborted by user"` |
///
/// Returns `AppError::NotFound` if no active run has the given `run_id`.
/// Returns `AppError::InvalidInput` if the channel is closed (the run already
/// exited or was never in step-failure mode).
#[tauri::command]
pub async fn respond_to_step_failure(
    run_id: String,
    choice: StepFailureChoice,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let sessions = state.run_manager.lock().await.sessions_arc();

    let handle = {
        let map = sessions.lock().await;
        map.get(&run_id).cloned()
    };

    let handle = match handle {
        None => return Err(AppError::NotFound(format!("run id: {}", run_id))),
        Some(h) => h,
    };

    handle
        .step_failure_tx
        .send(choice)
        .await
        .map_err(|_| AppError::InvalidInput(
            "run is not awaiting a step-failure response (channel closed)".to_string(),
        ))?;

    tracing::info!(run_id = %run_id, "respond_to_step_failure: choice sent");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::manager::SessionHandle;
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // is_valid_sequence_name
    // -----------------------------------------------------------------------

    /// Empty string must be rejected.
    #[test]
    fn is_valid_sequence_name_rejects_empty() {
        assert!(!is_valid_sequence_name(""), "empty string must be rejected");
    }

    /// A bare leading dash must be rejected.
    #[test]
    fn launch_run_rejects_sequence_name_with_leading_dash() {
        assert!(!is_valid_sequence_name("-"), "bare dash must be rejected");
        assert!(
            !is_valid_sequence_name("--print"),
            "\"--print\" must be rejected"
        );
        assert!(
            !is_valid_sequence_name("--help"),
            "\"--help\" must be rejected"
        );
        assert!(
            !is_valid_sequence_name("-x"),
            "single-letter flag must be rejected"
        );
    }

    /// Names that contain characters outside the allowlist must be rejected.
    #[test]
    fn is_valid_sequence_name_rejects_unsafe_characters() {
        assert!(
            !is_valid_sequence_name("foo;bar"),
            "semicolon must be rejected"
        );
        assert!(
            !is_valid_sequence_name("foo&bar"),
            "ampersand must be rejected"
        );
        assert!(!is_valid_sequence_name("foo|bar"), "pipe must be rejected");
        assert!(
            !is_valid_sequence_name("foo$bar"),
            "dollar sign must be rejected"
        );
        assert!(
            !is_valid_sequence_name("foo\nbar"),
            "newline must be rejected"
        );
        assert!(
            !is_valid_sequence_name("foo\x00bar"),
            "null byte must be rejected"
        );
    }

    /// Valid names must be accepted.
    #[test]
    fn is_valid_sequence_name_accepts_valid_names() {
        assert!(
            is_valid_sequence_name("my-sequence"),
            "hyphenated name must be accepted"
        );
        assert!(
            is_valid_sequence_name("sub/dir"),
            "forward-slash for subdir must be accepted"
        );
        assert!(
            is_valid_sequence_name("seq 1"),
            "space in name must be accepted"
        );
        assert!(
            is_valid_sequence_name("alpha.beta_gamma"),
            "dot and underscore must be accepted"
        );
        assert!(
            is_valid_sequence_name("MySequence"),
            "mixed case must be accepted"
        );
        assert!(is_valid_sequence_name("a"), "single char must be accepted");
        assert!(
            is_valid_sequence_name("foo.bar"),
            "dot in name must be accepted"
        );
    }

    /// Names containing `..` path components must be rejected.
    #[test]
    fn is_valid_sequence_name_rejects_dotdot_segments() {
        assert!(
            !is_valid_sequence_name("../../etc/passwd"),
            "double-dot traversal must be rejected"
        );
        assert!(
            !is_valid_sequence_name("../sibling"),
            "leading double-dot segment must be rejected"
        );
        assert!(
            !is_valid_sequence_name("sub/../etc"),
            "double-dot in middle must be rejected"
        );
        assert!(
            !is_valid_sequence_name(".."),
            "bare double-dot must be rejected"
        );
    }

    /// Single-dot segments and empty segments (trailing/consecutive slashes) must be rejected.
    #[test]
    fn is_valid_sequence_name_rejects_dot_and_empty_segments() {
        assert!(
            !is_valid_sequence_name("."),
            "bare single-dot must be rejected"
        );
        assert!(
            !is_valid_sequence_name("./foo"),
            "leading dot-segment must be rejected"
        );
        assert!(
            !is_valid_sequence_name("foo/."),
            "trailing dot-segment must be rejected"
        );
        assert!(
            !is_valid_sequence_name("foo//bar"),
            "consecutive slashes must be rejected"
        );
        assert!(
            !is_valid_sequence_name("foo/"),
            "trailing slash must be rejected"
        );
    }

    /// Names starting with `/` must be rejected as absolute paths.
    #[test]
    fn is_valid_sequence_name_rejects_absolute_path() {
        assert!(
            !is_valid_sequence_name("/absolute"),
            "leading slash must be rejected"
        );
        assert!(
            !is_valid_sequence_name("/etc/passwd"),
            "absolute path must be rejected"
        );
        assert!(!is_valid_sequence_name("/"), "bare slash must be rejected");
    }

    /// Names longer than 256 characters must be rejected.
    #[test]
    fn is_valid_sequence_name_rejects_over_256_chars() {
        let exactly_256 = "a".repeat(256);
        assert!(
            is_valid_sequence_name(&exactly_256),
            "exactly 256 chars must be accepted"
        );
        let over_256 = "a".repeat(257);
        assert!(
            !is_valid_sequence_name(&over_256),
            "257 chars must be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // send_input 64 KiB cap (guard predicate)
    // -----------------------------------------------------------------------

    /// Exactly 64 KiB must be accepted.
    #[test]
    fn send_input_size_guard_accepts_max_bytes() {
        const MAX_INPUT_BYTES: usize = 65_536;
        let text = "x".repeat(MAX_INPUT_BYTES);
        assert!(
            !(text.len() > MAX_INPUT_BYTES),
            "exactly 64 KiB must be accepted by the guard"
        );
    }

    /// One byte over the limit must be rejected.
    #[test]
    fn send_input_size_guard_rejects_over_max_bytes() {
        const MAX_INPUT_BYTES: usize = 65_536;
        let text = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(
            text.len() > MAX_INPUT_BYTES,
            "65537 bytes must be rejected by the guard"
        );
    }

    /// An empty string (0 bytes) must be accepted.
    #[test]
    fn send_input_size_guard_accepts_empty() {
        const MAX_INPUT_BYTES: usize = 65_536;
        let text = "";
        assert!(
            !(text.len() > MAX_INPUT_BYTES),
            "empty string must be accepted by the guard"
        );
    }

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
        assert!(
            result.is_err(),
            "bare filename must be rejected as relative"
        );
    }

    /// On Windows, a UNC path starting with `\\` must be rejected.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn validate_cli_path_rejects_unc_backslash() {
        let result = validate_cli_path(std::path::Path::new("\\\\server\\share\\claude")).await;
        assert!(
            result.is_err(),
            "UNC path with backslashes must be rejected"
        );
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
        let result = validate_cli_path(std::path::Path::new("//server/share/claude")).await;
        assert!(
            result.is_err(),
            "UNC path with forward slashes must be rejected"
        );
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
        assert!(
            result.is_ok(),
            "existing regular file must be accepted: {:?}",
            result
        );
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
    // T4.4 — read_attached_md
    // -----------------------------------------------------------------------

    /// A file that exists and is within the 1 MiB limit must be read successfully.
    #[tokio::test]
    async fn read_attached_md_reads_existing_file() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("context.md");
        let content = b"# Context\n\nThis is the attached context.\n";
        tokio::fs::write(&path, content)
            .await
            .expect("write test file");

        let result = read_attached_md(&path).await;
        assert!(
            result.is_ok(),
            "existing file must be readable: {:?}",
            result
        );
        assert_eq!(result.unwrap(), content, "content must match exactly");
    }

    /// A path that does not exist must return AppError::NotFound.
    #[tokio::test]
    async fn read_attached_md_returns_not_found_for_missing_file() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("nonexistent_context.md");

        let result = read_attached_md(&path).await;
        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "missing file must return NotFound, got: {:?}",
            result
        );
    }

    /// A file whose content exceeds 1 MiB must return AppError::InvalidInput.
    #[tokio::test]
    async fn read_attached_md_rejects_oversized_file() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("big.md");

        // Write exactly 1 MiB + 1 byte so it exceeds the cap.
        const LIMIT: usize = 1_048_576;
        let big = vec![b'x'; LIMIT + 1];
        tokio::fs::write(&path, &big).await.expect("write big file");

        let result = read_attached_md(&path).await;
        assert!(
            matches!(result, Err(AppError::InvalidInput(_))),
            "oversized file must return InvalidInput, got: {:?}",
            result
        );
    }

    /// A file at exactly the 1 MiB boundary must be accepted.
    #[tokio::test]
    async fn read_attached_md_accepts_file_at_exact_limit() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("exact_limit.md");

        const LIMIT: usize = 1_048_576;
        let exact = vec![b'a'; LIMIT];
        tokio::fs::write(&path, &exact)
            .await
            .expect("write limit file");

        let result = read_attached_md(&path).await;
        assert!(
            result.is_ok(),
            "file at exactly 1 MiB must be accepted: {:?}",
            result
        );
        assert_eq!(result.unwrap().len(), LIMIT);
    }

    /// An empty file (0 bytes) must be accepted and return an empty Vec.
    #[tokio::test]
    async fn read_attached_md_accepts_empty_file() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("empty.md");
        tokio::fs::write(&path, b"")
            .await
            .expect("write empty file");

        let result = read_attached_md(&path).await;
        assert!(result.is_ok(), "empty file must be accepted: {:?}", result);
        assert!(result.unwrap().is_empty(), "result must be empty Vec");
    }

    /// The error message for a missing file must mention the path.
    #[tokio::test]
    async fn read_attached_md_not_found_message_contains_path() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("missing_context.md");

        let result = read_attached_md(&path).await;
        match result {
            Err(AppError::NotFound(msg)) => {
                assert!(
                    msg.contains("missing_context.md"),
                    "NotFound message must contain the filename, got: {msg}"
                );
            }
            other => panic!("expected NotFound, got: {:?}", other),
        }
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
        // Build an empty sessions map (no active runs).
        let sessions: std::sync::Arc<
            tokio::sync::Mutex<HashMap<String, std::sync::Arc<SessionHandle>>>,
        > = std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new()));

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
        // Build an empty sessions map (no active runs).
        let sessions: std::sync::Arc<
            tokio::sync::Mutex<HashMap<String, std::sync::Arc<SessionHandle>>>,
        > = std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new()));

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

    // -----------------------------------------------------------------------
    // T4.7 — respond_to_step_failure lookup logic
    // -----------------------------------------------------------------------

    /// Helper: build a minimal `SessionHandle` with real channels for tests.
    fn make_test_handle() -> (
        std::sync::Arc<SessionHandle>,
        tokio::sync::mpsc::Receiver<crate::runs::StepFailureChoice>,
    ) {
        use crate::runs::{Run, RunStatus};
        use std::path::PathBuf;

        let (input_tx, _input_rx) = tokio::sync::mpsc::channel::<String>(1);
        let (sf_tx, sf_rx) = tokio::sync::mpsc::channel::<crate::runs::StepFailureChoice>(2);
        let run = Arc::new(tokio::sync::Mutex::new(Run {
            id: "test-run".to_string(),
            project_id: "proj".to_string(),
            project_path: PathBuf::from("/tmp"),
            sequence_name: "seq".to_string(),
            attached_md_path: None,
            started_at: chrono::Utc::now(),
            ended_at: None,
            status: RunStatus::Running,
            exit_code: None,
            pid: None,
            note: None,
            exit_note: None,
            retry_of: None,
        }));
        let handle = std::sync::Arc::new(SessionHandle {
            cancel: tokio_util::sync::CancellationToken::new(),
            stdin: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            run,
            input_tx,
            step_failure_tx: sf_tx,
        });
        (handle, sf_rx)
    }

    /// `respond_to_step_failure` returns `NotFound` for an unknown run_id.
    #[tokio::test]
    async fn respond_to_step_failure_returns_not_found_for_unknown_run_id() {
        let sessions: std::sync::Arc<
            tokio::sync::Mutex<HashMap<String, std::sync::Arc<SessionHandle>>>,
        > = std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let handle = {
            let map = sessions.lock().await;
            map.get("nonexistent").cloned()
        };

        let result: AppResult<()> = match handle {
            None => Err(AppError::NotFound("run id: nonexistent".to_string())),
            Some(h) => h
                .step_failure_tx
                .send(crate::runs::StepFailureChoice::Continue)
                .await
                .map_err(|_| {
                    AppError::InvalidInput("channel closed".to_string())
                }),
        };

        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "must return NotFound for unknown run_id; got: {:?}",
            result
        );
    }

    /// When a handle is present, sending a choice delivers it to the receiver.
    #[tokio::test]
    async fn respond_to_step_failure_delivers_choice_to_receiver() {
        let (handle, mut sf_rx) = make_test_handle();

        // Simulate the command handler sending a choice.
        let send_result = handle
            .step_failure_tx
            .send(crate::runs::StepFailureChoice::Retry)
            .await;
        assert!(send_result.is_ok(), "send must succeed: {:?}", send_result);

        // The receiver (I/O loop) gets the correct variant.
        let received = sf_rx.recv().await;
        assert!(
            matches!(received, Some(crate::runs::StepFailureChoice::Retry)),
            "receiver must get Retry; got: {:?}",
            received
        );
    }

    /// All four `StepFailureChoice` variants can be sent through the channel.
    #[tokio::test]
    async fn respond_to_step_failure_all_variants_deliverable() {
        use crate::runs::StepFailureChoice;

        let choices = [
            StepFailureChoice::Continue,
            StepFailureChoice::Retry,
            StepFailureChoice::Skip,
            StepFailureChoice::Abort,
        ];

        for choice in choices {
            let (handle, mut sf_rx) = make_test_handle();
            let choice_json = serde_json::to_string(&choice).unwrap();

            handle
                .step_failure_tx
                .send(choice)
                .await
                .expect("send must succeed");

            let received = sf_rx.recv().await.expect("receiver must get a value");
            let received_json = serde_json::to_string(&received).unwrap();
            assert_eq!(
                received_json, choice_json,
                "choice round-trip via channel failed"
            );
        }
    }

    /// Sending to a closed channel (receiver dropped) returns an error.
    #[tokio::test]
    async fn respond_to_step_failure_returns_error_when_channel_closed() {
        let (handle, sf_rx) = make_test_handle();
        // Drop the receiver to simulate the I/O loop having exited.
        drop(sf_rx);

        let result = handle
            .step_failure_tx
            .send(crate::runs::StepFailureChoice::Abort)
            .await;

        assert!(
            result.is_err(),
            "sending to a closed channel must return an error"
        );
    }
}
