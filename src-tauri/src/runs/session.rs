// RunSession — per-run state machine and background I/O loop.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use tauri::Emitter;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::ipc::events;
use super::parser::EventParser;
use super::transcript::TranscriptWriter;
use super::{Run, RunStatus};
use super::manager::SessionHandle;

// ---------------------------------------------------------------------------
// Public helpers (also used by launch_run command)
// ---------------------------------------------------------------------------

/// Returns true iff every character in `run_id` is `[A-Za-z0-9._-]`.
///
/// This ensures the run_id can be used as a directory name without path
/// traversal or shell injection risk.
pub fn validate_run_id(run_id: &str) -> bool {
    if run_id.is_empty() {
        return false;
    }
    run_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Build the run directory path: `<project_path>/.claude/runs/<run_id>/`.
pub fn build_run_dir(project_path: &Path, run_id: &str) -> PathBuf {
    project_path.join(".claude").join("runs").join(run_id)
}

/// Canonicalize `project_path` and verify that `run_dir` is rooted under
/// `<canonical_project_path>/.claude/runs/`.
///
/// Returns `AppError::PermissionDenied` if the resolved `run_dir` is outside
/// the expected prefix.  This guards against directory traversal if a caller
/// somehow produces a malformed `run_id`.
pub async fn verify_run_dir_prefix(project_path: &Path, run_dir: &Path) -> AppResult<()> {
    // Canonicalize the project root.  On Windows this also resolves 8.3 names.
    let canonical_project = tokio::fs::canonicalize(project_path).await.map_err(|e| {
        AppError::PermissionDenied(format!(
            "cannot canonicalize project path '{}': {}",
            project_path.display(),
            e
        ))
    })?;

    // Build the expected prefix string: <canonical>/.claude/runs/
    let expected_prefix = canonical_project
        .join(".claude")
        .join("runs")
        .to_string_lossy()
        .into_owned();

    // run_dir may not exist yet (created by TranscriptWriter::create).
    // We verify lexically after normalizing separators on Windows.
    let run_dir_str = run_dir.to_string_lossy().into_owned();

    // Normalize Windows path separators for the prefix check.
    let run_dir_norm = run_dir_str.replace('\\', "/");
    let prefix_norm = expected_prefix.replace('\\', "/");

    if !run_dir_norm.starts_with(&prefix_norm) {
        return Err(AppError::PermissionDenied(format!(
            "run_dir '{}' is outside the expected prefix '{}'",
            run_dir.display(),
            expected_prefix,
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Background I/O loop
// ---------------------------------------------------------------------------

/// Spawnable background task that:
/// 1. Updates run status to `Running` and emits `run:started`.
/// 2. Streams stdout through `EventParser`, writing events to transcript and
///    emitting `run:event` to the frontend.
/// 3. Appends stderr bytes to `raw.log`.
/// 4. On cancellation, kills the child.
/// 5. After the child exits, finalises the run status and emits `run:finished`.
/// 6. Removes the session from the `sessions` map.
#[allow(clippy::too_many_arguments)]
pub async fn run_io_loop(
    app_handle: tauri::AppHandle,
    run_id: String,
    run: Arc<Mutex<Run>>,
    mut stdout: tokio::process::ChildStdout,
    mut stderr: tokio::process::ChildStderr,
    mut child: tokio::process::Child,
    mut writer: TranscriptWriter,
    cancel: CancellationToken,
    sessions: Arc<Mutex<HashMap<String, Arc<SessionHandle>>>>,
) {
    let span = tracing::info_span!("run_session", run_id = %run_id);
    let _enter = span.enter();

    // ── Step 1: mark Running, write meta, emit run:started ──────────────────

    let project_id = {
        let mut run_guard = run.lock().await;
        run_guard.status = RunStatus::Running;
        run_guard.pid = child.id();
        let snapshot = run_guard.clone();
        drop(run_guard);

        let pid = snapshot.pid;
        let project_id = snapshot.project_id.clone();

        tracing::info!(
            run_id = %run_id,
            project_id = %project_id,
            pid = ?pid,
            "run started"
        );

        if let Err(e) = writer.update_meta(&snapshot).await {
            tracing::warn!(run_id = %run_id, error = %e, "failed to write meta on start");
        }

        project_id
    };

    let started_payload = serde_json::json!({ "run_id": run_id, "project_id": project_id });
    if let Err(e) = app_handle.emit(events::RUN_STARTED, started_payload) {
        tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:started");
    }

    // ── Step 2: I/O loop ────────────────────────────────────────────────────

    let mut parser = EventParser::new();
    let mut stdout_buf = vec![0u8; 8192];
    let mut stderr_buf = vec![0u8; 8192];
    let mut cancelled = false;

    loop {
        tokio::select! {
            // Read stdout.
            result = stdout.read(&mut stdout_buf) => {
                match result {
                    Ok(0) => {
                        // EOF on stdout — child has closed its write end.
                        break;
                    }
                    Ok(n) => {
                        let chunk = &stdout_buf[..n];
                        // Append to raw log.
                        if let Err(e) = writer.append_raw(chunk).await {
                            tracing::warn!(run_id = %run_id, error = %e, "append_raw stdout failed");
                        }
                        // Feed to parser, emit events.
                        let events_parsed = parser.feed(chunk);
                        for event in &events_parsed {
                            if let Err(e) = writer.append_event(event).await {
                                tracing::warn!(run_id = %run_id, error = %e, "append_event failed");
                            }
                            let payload = serde_json::json!({ "run_id": run_id, "event": event });
                            if let Err(e) = app_handle.emit(events::RUN_EVENT, payload) {
                                tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:event");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(run_id = %run_id, error = %e, "stdout read error");
                        break;
                    }
                }
            }

            // Read stderr.
            result = stderr.read(&mut stderr_buf) => {
                match result {
                    Ok(0) => {
                        // stderr EOF — keep looping for stdout.
                    }
                    Ok(n) => {
                        let chunk = &stderr_buf[..n];
                        if let Err(e) = writer.append_raw(chunk).await {
                            tracing::warn!(run_id = %run_id, error = %e, "append_raw stderr failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(run_id = %run_id, error = %e, "stderr read error");
                    }
                }
            }

            // Cancellation.
            _ = cancel.cancelled() => {
                cancelled = true;
                tracing::info!(run_id = %run_id, "run cancellation requested — killing child");
                if let Err(e) = child.kill().await {
                    tracing::warn!(run_id = %run_id, error = %e, "child kill failed");
                }
                break;
            }
        }
    }

    // ── Step 3: drain remaining output (best-effort, with short timeout) ────

    if !cancelled {
        let drain_timeout = std::time::Duration::from_millis(200);

        // Drain stdout.
        let _ = tokio::time::timeout(drain_timeout, async {
            loop {
                match stdout.read(&mut stdout_buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let chunk = &stdout_buf[..n];
                        let _ = writer.append_raw(chunk).await;
                        let events_parsed = parser.feed(chunk);
                        for event in &events_parsed {
                            let _ = writer.append_event(event).await;
                            let payload = serde_json::json!({ "run_id": run_id, "event": event });
                            let _ = app_handle.emit(events::RUN_EVENT, payload);
                        }
                    }
                }
            }
        })
        .await;

        // Drain stderr.
        let _ = tokio::time::timeout(drain_timeout, async {
            loop {
                match stderr.read(&mut stderr_buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let _ = writer.append_raw(&stderr_buf[..n]).await;
                    }
                }
            }
        })
        .await;
    }

    // ── Step 4: wait for child exit ──────────────────────────────────────────

    let exit_status = child.wait().await;
    let exit_code: Option<i32> = exit_status
        .ok()
        .and_then(|s| s.code());

    // ── Step 5: determine final status ──────────────────────────────────────

    let final_status = if cancelled {
        RunStatus::Stopped
    } else if exit_code == Some(0) {
        RunStatus::Completed
    } else {
        RunStatus::Failed
    };

    tracing::info!(
        run_id = %run_id,
        status = ?final_status,
        exit_code = ?exit_code,
        "run finished"
    );

    // ── Step 6: update run meta and close writer ─────────────────────────────

    {
        let mut run_guard = run.lock().await;
        run_guard.status = final_status.clone();
        run_guard.ended_at = Some(Utc::now());
        run_guard.exit_code = exit_code;
        let snapshot = run_guard.clone();
        drop(run_guard);

        if let Err(e) = writer.update_meta(&snapshot).await {
            tracing::warn!(run_id = %run_id, error = %e, "failed to write final meta");
        }
    }

    if let Err(e) = writer.close().await {
        tracing::warn!(run_id = %run_id, error = %e, "transcript writer close failed");
    }

    // ── Step 7: emit run:finished ────────────────────────────────────────────

    let finished_payload = serde_json::json!({
        "run_id": run_id,
        "status": final_status,
        "exit_code": exit_code,
    });
    if let Err(e) = app_handle.emit(events::RUN_FINISHED, finished_payload) {
        tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:finished");
    }

    // ── Step 8: remove from sessions map ────────────────────────────────────

    {
        let mut map = sessions.lock().await;
        map.remove(&run_id);
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_run_id ──────────────────────────────────────────────────────

    /// UUID v7 hyphenated format (all `[a-f0-9-]`) must pass.
    #[test]
    fn validate_run_id_accepts_uuid() {
        let uuid = uuid::Uuid::now_v7().to_string(); // e.g. "019500000000-0000-0000-0000-000000000000"
        assert!(
            validate_run_id(&uuid),
            "UUID v7 hyphenated string should be valid, got: {}",
            uuid
        );
    }

    /// Path traversal patterns must be rejected.
    #[test]
    fn validate_run_id_rejects_slashes() {
        assert!(!validate_run_id("../etc/passwd"), "path traversal must be rejected");
        assert!(!validate_run_id("/absolute/path"), "absolute path must be rejected");
        assert!(!validate_run_id("foo/bar"), "forward slash must be rejected");
        assert!(!validate_run_id("foo\\bar"), "backslash must be rejected");
    }

    /// An empty string must be rejected.
    #[test]
    fn validate_run_id_rejects_empty() {
        assert!(!validate_run_id(""), "empty string must be rejected");
    }

    /// A string containing a space must be rejected.
    #[test]
    fn validate_run_id_rejects_spaces() {
        assert!(!validate_run_id("my run"), "space must be rejected");
        assert!(!validate_run_id(" leading"), "leading space must be rejected");
        assert!(!validate_run_id("trailing "), "trailing space must be rejected");
    }

    // ── build_run_dir ────────────────────────────────────────────────────────

    /// `build_run_dir` must construct `<project_path>/.claude/runs/<run_id>/`.
    #[test]
    fn build_run_dir_constructs_correct_path() {
        let project = Path::new("/projects/foo");
        let result = build_run_dir(project, "abc-123");
        // The path must end with `.claude/runs/abc-123`.
        let components: Vec<_> = result.components().collect();
        // Find ".claude/runs/abc-123" suffix.
        assert!(
            result.ends_with(Path::new(".claude/runs/abc-123")),
            "expected path to end with .claude/runs/abc-123, got: {}",
            result.display()
        );
        // Should have more components than just the suffix (project root is prepended).
        assert!(components.len() >= 4, "path too short: {:?}", result);
    }

    // ── verify_run_dir_prefix ────────────────────────────────────────────────

    /// A legitimate run_dir inside the project's `.claude/runs/` passes.
    #[tokio::test]
    async fn verify_run_dir_prefix_accepts_valid() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let project_path = tmp.path();

        // Ensure the run_dir exists so canonicalize of the project_path works.
        // (run_dir itself doesn't need to exist — we only canonicalize project_path.)
        let run_dir = build_run_dir(project_path, "valid-run-id");
        let result = verify_run_dir_prefix(project_path, &run_dir).await;
        assert!(
            result.is_ok(),
            "valid run_dir inside project should be accepted, got: {:?}",
            result
        );
    }

    /// A path outside the project prefix must return `PermissionDenied`.
    #[tokio::test]
    async fn verify_run_dir_prefix_rejects_traversal() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let project_path = tmp.path();

        // Craft a path that is NOT under <project>/.claude/runs/.
        let bad_run_dir = tmp.path().join("..").join("outside");
        let result = verify_run_dir_prefix(project_path, &bad_run_dir).await;
        assert!(
            matches!(result, Err(AppError::PermissionDenied(_))),
            "path outside project prefix must return PermissionDenied, got: {:?}",
            result
        );
    }
}
