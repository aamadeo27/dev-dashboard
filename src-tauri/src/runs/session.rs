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
// RunIoContext — bundles all per-run parameters for run_io_loop
// ---------------------------------------------------------------------------

/// All context needed by the background I/O loop for a single run.
///
/// Grouping these into a struct keeps `run_io_loop`'s signature short and
/// makes the call site in `commands.rs` self-documenting.
pub(crate) struct RunIoContext {
    pub(crate) run: Arc<Mutex<Run>>,
    pub(crate) stdout: tokio::process::ChildStdout,
    pub(crate) stderr: tokio::process::ChildStderr,
    pub(crate) child: tokio::process::Child,
    pub(crate) writer: TranscriptWriter,
    pub(crate) cancel: CancellationToken,
    pub(crate) sessions: Arc<Mutex<HashMap<String, Arc<SessionHandle>>>>,
    pub(crate) input_rx: tokio::sync::mpsc::Receiver<String>,
}

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

/// Resolve `..` components in a forward-slash-normalised path string without
/// touching the filesystem.  Empty and `.` components are dropped; `..`
/// components pop the last accumulated segment (no-op at the root).
///
/// This is applied to `run_dir` inside `verify_run_dir_prefix` so that a path
/// like `<project>/.claude/runs/../../outside` cannot bypass the prefix check.
fn normalize_path_lexically(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => { parts.pop(); }
            p => parts.push(p),
        }
    }
    parts.join("/")
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
    // Normalize Windows path separators, then collapse any `..` components
    // lexically so a path like `<project>/.claude/runs/../../outside` cannot
    // bypass the prefix check.
    let run_dir_str = run_dir.to_string_lossy().into_owned();
    let run_dir_norm = normalize_path_lexically(&run_dir_str.replace('\\', "/"));

    // Append a trailing "/" so that a sibling dir like `.claude/runs-evil/`
    // cannot falsely pass as a prefix of `.claude/runs/`.
    let prefix_norm = format!("{}/", expected_prefix.replace('\\', "/"));

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
// Background I/O helpers
// ---------------------------------------------------------------------------

/// Append a raw stdout/stderr chunk, parse it for structured events, write all
/// parsed events to the transcript in one batch, and emit a single `run:event`
/// payload to the frontend.
///
/// Called from both the main select! loop and the post-cancel drain block so
/// that the identical processing logic is not duplicated.
async fn process_stdout_chunk(
    chunk: &[u8],
    run_id: &str,
    writer: &mut TranscriptWriter,
    parser: &mut EventParser,
    app_handle: &tauri::AppHandle,
) {
    if let Err(e) = writer.append_raw(chunk).await {
        tracing::warn!(run_id = %run_id, error = %e, "append_raw stdout failed");
    }
    let events_parsed = parser.feed(chunk);
    if let Err(e) = writer.append_events(&events_parsed).await {
        tracing::warn!(run_id = %run_id, error = %e, "append_events failed");
    }
    if !events_parsed.is_empty() {
        let payload = serde_json::json!({ "run_id": run_id, "events": events_parsed });
        if let Err(e) = app_handle.emit(events::RUN_EVENT, payload) {
            tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:event");
        }
    }
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
///
/// The span is instrumented at the `tokio::task::spawn` call site in
/// `commands.rs` via `.instrument(span)` so that the async-aware
/// tracing span is entered/exited correctly across await points.
pub async fn run_io_loop(app_handle: tauri::AppHandle, run_id: String, ctx: RunIoContext) {
    let RunIoContext {
        run,
        mut stdout,
        mut stderr,
        mut child,
        mut writer,
        cancel,
        sessions,
        mut input_rx,
    } = ctx;

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
                        // Batch raw append + parse + transcript write + emit.
                        process_stdout_chunk(chunk, &run_id, &mut writer, &mut parser, &app_handle).await;
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

            // UserInput channel — write to transcript and emit run:event.
            // Not in the hot path so we emit immediately (single-event array).
            maybe_text = input_rx.recv() => {
                if let Some(text) = maybe_text {
                    let event = super::RunEvent::UserInput {
                        text,
                        ts: chrono::Utc::now(),
                    };
                    if let Err(e) = writer.append_event(&event).await {
                        tracing::warn!(run_id = %run_id, error = %e, "append_event UserInput failed");
                    }
                    let payload = serde_json::json!({ "run_id": run_id, "events": [event] });
                    if let Err(e) = app_handle.emit(events::RUN_EVENT, payload) {
                        tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:event UserInput");
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
    // Always drain regardless of cancellation: the child may flush final output
    // after kill() before the pipe closes.  The 200 ms timeout bounds this.
    // stdout and stderr are interleaved via tokio::select! so neither stream
    // can starve the other within the shared budget.

    let drain_timeout = std::time::Duration::from_millis(200);
    let mut stdout_done = false;
    let mut stderr_done = false;

    let _ = tokio::time::timeout(drain_timeout, async {
        loop {
            if stdout_done && stderr_done {
                break;
            }
            tokio::select! {
                result = stdout.read(&mut stdout_buf), if !stdout_done => {
                    match result {
                        Ok(0) | Err(_) => stdout_done = true,
                        Ok(n) => {
                            process_stdout_chunk(&stdout_buf[..n], &run_id, &mut writer, &mut parser, &app_handle).await;
                        }
                    }
                }
                result = stderr.read(&mut stderr_buf), if !stderr_done => {
                    match result {
                        Ok(0) | Err(_) => stderr_done = true,
                        Ok(n) => {
                            let _ = writer.append_raw(&stderr_buf[..n]).await;
                        }
                    }
                }
            }
        }
    })
    .await;

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

    // ── T4.3 gap tests ───────────────────────────────────────────────────────

    /// All allowed punctuation characters (`.`, `_`, `-`) combined with
    /// alphanumerics must be accepted.
    ///
    /// Covers the positive boundary that the existing UUID test (`[a-f0-9-]`)
    /// does not exercise: upper-case letters, digits, dot, and underscore.
    #[test]
    fn validate_run_id_accepts_valid_chars() {
        assert!(
            validate_run_id("my.run_id-001"),
            "alphanumerics + dot + underscore + hyphen must be accepted"
        );
        assert!(
            validate_run_id("ABC"),
            "upper-case letters must be accepted"
        );
        assert!(
            validate_run_id("a"),
            "single-char id must be accepted"
        );
        assert!(
            validate_run_id("Z9.x_y-z"),
            "mixed valid chars must be accepted"
        );
    }

    /// A run_id containing a non-ASCII Unicode character must be rejected.
    ///
    /// `ü` is U+00FC — it passes `char::is_alphanumeric()` but fails
    /// `char::is_ascii_alphanumeric()`, so this guards the correct predicate
    /// is used.
    #[test]
    fn validate_run_id_rejects_unicode() {
        assert!(
            !validate_run_id("rün"),
            "non-ASCII unicode character must be rejected"
        );
        assert!(
            !validate_run_id("日本語"),
            "CJK characters must be rejected"
        );
        assert!(
            !validate_run_id("naïve"),
            "latin-1 supplement must be rejected"
        );
    }

    /// A run_id containing a null byte (`\0`) must be rejected.
    ///
    /// Null bytes are not printable ASCII and must never appear in a path
    /// component; `is_ascii_alphanumeric()` already rejects them but this
    /// test makes the intent explicit and guards against future refactors.
    #[test]
    fn validate_run_id_rejects_null_byte() {
        assert!(
            !validate_run_id("run\0id"),
            "embedded null byte must be rejected"
        );
        assert!(
            !validate_run_id("\0"),
            "lone null byte must be rejected"
        );
    }

    /// `build_run_dir` must embed the `.claude/runs/` segment between the
    /// project root and the run id.
    ///
    /// This is a stronger assertion than `build_run_dir_constructs_correct_path`
    /// (which only checks the suffix) — it also verifies the intermediate
    /// components, ensuring the directory is nested under `.claude` then `runs`.
    #[test]
    fn build_run_dir_uses_claude_subdir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let project_path = tmp.path();
        let run_id = "test-run-42";

        let result = build_run_dir(project_path, run_id);

        // The path must contain `.claude` as a component.
        let components: Vec<String> = result
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();

        let claude_pos = components.iter().position(|c| c == ".claude");
        assert!(
            claude_pos.is_some(),
            "path must contain '.claude' component, got components: {:?}",
            components
        );

        let claude_pos = claude_pos.unwrap();

        // `.claude` must be immediately followed by `runs`.
        assert_eq!(
            components.get(claude_pos + 1).map(|s| s.as_str()),
            Some("runs"),
            "'.claude' must be immediately followed by 'runs', components: {:?}",
            components
        );

        // `runs` must be immediately followed by the run_id.
        assert_eq!(
            components.get(claude_pos + 2).map(|s| s.as_str()),
            Some(run_id),
            "'runs' must be immediately followed by the run_id, components: {:?}",
            components
        );
    }

    /// A path under `.claude/runs-evil/` (sibling of `.claude/runs/`) must be
    /// rejected.  Without the trailing-slash fix the old `starts_with` check
    /// would have falsely accepted this path because `runs-evil` has `runs` as
    /// a prefix.
    #[tokio::test]
    async fn verify_run_dir_prefix_rejects_runs_sibling_dir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let project_path = tmp.path();

        // Construct a path that sits under `.claude/runs-evil/` — a sibling of
        // the legitimate `.claude/runs/` directory.
        let evil_run_dir = project_path.join(".claude").join("runs-evil").join("abc");
        let result = verify_run_dir_prefix(project_path, &evil_run_dir).await;

        assert!(
            matches!(result, Err(AppError::PermissionDenied(_))),
            "path under .claude/runs-evil/ must be rejected with PermissionDenied, got: {:?}",
            result
        );
    }

    /// A `run_dir` containing `../..` that escapes the project must be rejected
    /// even though the string contains the runs prefix as a substring.
    ///
    /// The path `<project>/.claude/runs/../../outside` resolves to
    /// `<project>/outside` after lexical normalisation — clearly outside the
    /// expected prefix.
    #[tokio::test]
    async fn verify_run_dir_prefix_rejects_dotdot_in_run_dir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let project_path = tmp.path();

        // Craft a path that starts inside .claude/runs/ but then escapes with ../..
        let dotdot_run_dir = project_path
            .join(".claude")
            .join("runs")
            .join("..")
            .join("..")
            .join("outside");

        let result = verify_run_dir_prefix(project_path, &dotdot_run_dir).await;
        assert!(
            matches!(result, Err(AppError::PermissionDenied(_))),
            "run_dir with .. escape must return PermissionDenied, got: {:?}",
            result
        );
    }

    /// A run_dir under a *different* project directory (sibling) must be
    /// rejected even though it would pass a non-canonical prefix check.
    ///
    /// Two separate `TempDir`s are created (A and B).  The `run_dir` is
    /// constructed under B.  When `verify_run_dir_prefix` is called with
    /// project_path = A, it must return `PermissionDenied`.
    ///
    /// This covers the security boundary that the traversal test cannot
    /// exercise: the path is syntactically valid but belongs to the wrong tree.
    #[tokio::test]
    async fn verify_run_dir_prefix_rejects_sibling_project() {
        let tmp_a = tempfile::TempDir::new().expect("tempdir A");
        let tmp_b = tempfile::TempDir::new().expect("tempdir B");

        let project_path_a = tmp_a.path();
        // Build a run_dir that is legitimately under B, not A.
        let run_dir_under_b = build_run_dir(tmp_b.path(), "run-under-b");

        let result = verify_run_dir_prefix(project_path_a, &run_dir_under_b).await;

        assert!(
            matches!(result, Err(AppError::PermissionDenied(_))),
            "run_dir under a sibling project must be rejected with PermissionDenied, got: {:?}",
            result
        );
    }
}
