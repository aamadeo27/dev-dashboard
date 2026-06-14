// RunSession — per-run state machine and background I/O loop.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tauri::Emitter;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::manager::SessionHandle;
use super::parser::EventParser;
use super::transcript::TranscriptWriter;
use super::{LaunchInput, Run, RunStatus, StepFailureChoice};
use crate::error::{AppError, AppResult};
use crate::ipc::events;

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
    /// Original launch parameters — needed for re-invocation after step-failure.
    pub(crate) launch_input: LaunchInput,
    /// Resolved CLI path — reused for re-invocation without re-querying settings.
    pub(crate) cli_path: PathBuf,
    /// Shared stdin handle (same Arc as `SessionHandle.stdin`) used by the
    /// Continue choice to write "\n" without going through the IPC layer.
    pub(crate) stdin_arc: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// Receiver half of the step-failure response channel.
    /// `respond_to_step_failure` pushes the user's `StepFailureChoice` here.
    pub(crate) step_failure_rx: tokio::sync::mpsc::Receiver<StepFailureChoice>,
    /// Pre-read content of `launch_input.attached_md_path`, if any.
    /// Stored here so re-invocations can write it to the new child's stdin
    /// without re-reading the file.
    pub(crate) attached_md_content: Option<Vec<u8>>,
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
    run_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Build the run directory path: `<project_path>/.claude/runs/<run_id>/`.
pub fn build_run_dir(project_path: &Path, run_id: &str) -> PathBuf {
    project_path.join(".claude").join("runs").join(run_id)
}

/// Resolve `.` and `..` components in a path lexically (without touching the
/// filesystem), preserving any prefix/root component.  `.` is dropped; `..`
/// pops the last normal segment.
///
/// Operating on `Path` components (rather than a slash-joined string) keeps the
/// platform separator and any Windows `\\?\` verbatim prefix intact, so the
/// result can be compared with [`Path::starts_with`].
///
/// This is applied to `run_dir` inside `verify_run_dir_prefix` so that a path
/// like `<project>/.claude/runs/../../outside` cannot bypass the prefix check.
fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
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

    // The expected prefix: <canonical>/.claude/runs
    let expected_prefix = canonical_project.join(".claude").join("runs");

    // run_dir may not exist yet (created by TranscriptWriter::create), so it
    // cannot be canonicalized directly. Re-root it onto the canonical project
    // path: canonicalize() can rewrite the project path (Windows `\\?\` verbatim
    // prefix, macOS /var -> /private/var symlink), so a freshly-joined run_dir
    // built from the original project_path would otherwise never share the
    // canonical prefix. A run_dir outside the project (strip_prefix fails) is
    // left untouched and will fail the prefix check below.
    let run_dir_rebased = match run_dir.strip_prefix(project_path) {
        Ok(rel) => canonical_project.join(rel),
        Err(_) => run_dir.to_path_buf(),
    };

    // Collapse any `..`/`.` components lexically so a path like
    // `<project>/.claude/runs/../../outside` cannot bypass the prefix check.
    // `Path::starts_with` matches whole components, so a sibling dir such as
    // `.claude/runs-evil/` is correctly rejected (its `runs-evil` component
    // does not equal `runs`).
    let run_dir_norm = normalize_path_lexically(&run_dir_rebased);

    if !run_dir_norm.starts_with(&expected_prefix) {
        return Err(AppError::PermissionDenied(format!(
            "run_dir '{}' is outside the expected prefix '{}'",
            run_dir.display(),
            expected_prefix.display(),
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
/// Returns `true` if any `StepFailed` event was found in the parsed batch.
///
/// Called from both the main select! loop and the post-cancel drain block so
/// that the identical processing logic is not duplicated.
async fn process_stdout_chunk(
    chunk: &[u8],
    run_id: &str,
    writer: &mut TranscriptWriter,
    parser: &mut EventParser,
    app_handle: &tauri::AppHandle,
) -> bool {
    if let Err(e) = writer.append_raw(chunk).await {
        tracing::warn!(run_id = %run_id, error = %e, "append_raw stdout failed");
    }
    let events_parsed = parser.feed(chunk);
    let had_step_failed = events_parsed
        .iter()
        .any(|e| matches!(e, super::RunEvent::StepFailed { .. }));
    if let Err(e) = writer.append_events(&events_parsed).await {
        tracing::warn!(run_id = %run_id, error = %e, "append_events failed");
    }
    if !events_parsed.is_empty() {
        let payload = serde_json::json!({ "run_id": run_id, "events": events_parsed });
        if let Err(e) = app_handle.emit(events::RUN_EVENT, payload) {
            tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:event");
        }
    }
    had_step_failed
}

// ---------------------------------------------------------------------------
// Re-invocation helper
// ---------------------------------------------------------------------------

/// Prepare a re-invocation of the Claude CLI child process with `new_input`.
///
/// Creates a fresh run directory, mints a new `run_id`, records
/// `retry_of = original_run_id` in the new run's `meta.json`, writes
/// `attached_md_content` to the new child's stdin (if provided), and inserts
/// the new `SessionHandle` into `sessions`.
///
/// Returns `(new_run_id, RunIoContext)` on success.  The **caller** is
/// responsible for spawning `run_io_loop` with the returned context — this
/// keeps `re_invoke_run` free of any recursive `tokio::task::spawn` calls,
/// which would create a circular `Send` bound that the Rust compiler cannot
/// resolve statically.
///
/// Returns `None` on error (errors are logged; the caller exits its own loop).
async fn re_invoke_run(
    original_run_id: &str,
    project_path: &Path,
    new_input: LaunchInput,
    cli_path: &Path,
    attached_md_content: &Option<Vec<u8>>,
    sessions: Arc<Mutex<HashMap<String, Arc<SessionHandle>>>>,
) -> Option<(String, RunIoContext)> {
    use std::process::Stdio;

    // 1. Mint a new run_id.
    let new_run_id = uuid::Uuid::now_v7().to_string();

    // 2. Build and verify the new run directory.
    let run_dir = build_run_dir(project_path, &new_run_id);
    if let Err(e) = verify_run_dir_prefix(project_path, &run_dir).await {
        tracing::error!(
            original_run_id = %original_run_id,
            new_run_id = %new_run_id,
            error = %e,
            "re_invoke_run: run_dir prefix check failed"
        );
        return None;
    }

    // 3. Build the initial Run record for the new run.
    let initial_run = Run {
        id: new_run_id.clone(),
        project_id: new_input.project_id.clone(),
        project_path: project_path.to_path_buf(),
        sequence_name: new_input.sequence_name.clone(),
        attached_md_path: new_input.attached_md_path.clone(),
        started_at: Utc::now(),
        ended_at: None,
        status: RunStatus::Pending,
        exit_code: None,
        pid: None,
        note: None,
        exit_note: None,
        retry_of: Some(original_run_id.to_string()),
    };

    // 4. Create transcript writer (creates run_dir and files).
    let writer = match TranscriptWriter::create(&new_run_id, &run_dir, &initial_run).await {
        Ok(w) => w,
        Err(e) => {
            tracing::error!(
                original_run_id = %original_run_id,
                new_run_id = %new_run_id,
                error = %e,
                "re_invoke_run: TranscriptWriter::create failed"
            );
            return None;
        }
    };

    // 5. Spawn the new child process.
    let mut cmd = tokio::process::Command::new(cli_path);
    cmd.arg(&new_input.sequence_name)
        .current_dir(project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                original_run_id = %original_run_id,
                new_run_id = %new_run_id,
                error = %e,
                "re_invoke_run: child spawn failed"
            );
            return None;
        }
    };

    let mut child_stdin = child.stdin.take();
    let child_stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::error!(new_run_id = %new_run_id, "re_invoke_run: child stdout missing");
            return None;
        }
    };
    let child_stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            tracing::error!(new_run_id = %new_run_id, "re_invoke_run: child stderr missing");
            return None;
        }
    };

    // 6. Write attached_md content to the new child's stdin.
    if let Some(ref content) = attached_md_content {
        if !content.is_empty() {
            if let Some(ref mut stdin) = child_stdin {
                if let Err(e) = stdin.write_all(content).await {
                    tracing::warn!(
                        new_run_id = %new_run_id,
                        error = %e,
                        "re_invoke_run: failed to write attached_md to stdin"
                    );
                } else if let Err(e) = stdin.write_all(b"\n").await {
                    tracing::warn!(
                        new_run_id = %new_run_id,
                        error = %e,
                        "re_invoke_run: failed to write attached_md newline to stdin"
                    );
                }
            }
        }
    }

    // 7. Build channels and SessionHandle for the new run.
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

    // 8. Insert handle into sessions map BEFORE returning so the new run is
    //    visible to IPC commands as soon as the caller spawns the I/O task.
    {
        let mut map = sessions.lock().await;
        map.insert(new_run_id.clone(), handle);
    }

    // 9. Build and return the RunIoContext; the caller will spawn the I/O loop.
    let ctx = RunIoContext {
        run: run_arc,
        stdout: child_stdout,
        stderr: child_stderr,
        child,
        writer,
        cancel: cancel_token,
        sessions: sessions.clone(),
        input_rx,
        launch_input: new_input,
        cli_path: cli_path.to_path_buf(),
        stdin_arc,
        step_failure_rx: sf_rx,
        attached_md_content: attached_md_content.clone(),
    };

    tracing::info!(
        original_run_id = %original_run_id,
        new_run_id = %new_run_id,
        "re_invoke_run: context prepared, new run ready to spawn"
    );

    Some((new_run_id, ctx))
}

// ---------------------------------------------------------------------------
// Background I/O loop
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Step-failure handler
// ---------------------------------------------------------------------------

/// Outcome returned by [`handle_step_failure`] to the I/O loop.
enum StepFailureOutcome {
    /// Child responded to the Continue newline — keep the outer loop running.
    Continue,
    /// Exit the outer I/O loop with the given state.
    Break {
        cancelled: bool,
        exit_note: Option<String>,
    },
}

/// Handle the step-failure protocol after a `StepFailed` event is detected.
///
/// Emits `run:step_failure`, waits up to 60 s for a [`StepFailureChoice`]
/// (draining stdout/stderr in the meantime so the child does not block on a
/// full pipe buffer), then applies the per-choice kill + re-invoke action per
/// KB §9 item 3 (T4.8 option b).
///
/// Returns [`StepFailureOutcome`] so the caller can update `cancelled` /
/// `exit_note` and decide whether to `break 'io`.
#[allow(clippy::too_many_arguments)]
async fn handle_step_failure(
    run_id: &str,
    app_handle: &tauri::AppHandle,
    stdout: &mut tokio::process::ChildStdout,
    stderr: &mut tokio::process::ChildStderr,
    child: &mut tokio::process::Child,
    writer: &mut TranscriptWriter,
    parser: &mut EventParser,
    stdin_arc: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    step_failure_rx: &mut tokio::sync::mpsc::Receiver<StepFailureChoice>,
    cancel: &tokio_util::sync::CancellationToken,
    launch_input: &LaunchInput,
    cli_path: &Path,
    attached_md_content: &Option<Vec<u8>>,
    sessions: Arc<Mutex<HashMap<String, Arc<SessionHandle>>>>,
    project_path: &Path,
) -> StepFailureOutcome {
    tracing::info!(run_id = %run_id, "step failure detected");

    // 1. Emit run:step_failure so the UI can prompt the user.
    let sf_payload = serde_json::json!({ "run_id": run_id });
    if let Err(e) = app_handle.emit(events::RUN_STEP_FAILURE, sf_payload) {
        tracing::warn!(run_id = %run_id, error = %e, "failed to emit run:step_failure");
    }

    // 2. Wait up to 60 s for a choice, draining stdout/stderr while waiting.
    let mut stdout_buf = vec![0u8; 8192];
    let mut stderr_buf = vec![0u8; 8192];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);

    let choice = 'sf_wait: loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            tracing::info!(run_id = %run_id, "step_failure: auto-Continue after 60 s timeout");
            break 'sf_wait StepFailureChoice::Continue;
        }

        tokio::select! {
            maybe_choice = step_failure_rx.recv() => {
                break 'sf_wait maybe_choice.unwrap_or(StepFailureChoice::Continue);
            }
            result = stdout.read(&mut stdout_buf) => {
                match result {
                    Ok(0) | Err(_) => {
                        // Child exited while waiting — treat as Continue and let
                        // the outer loop detect EOF on next iteration.
                        break 'sf_wait StepFailureChoice::Continue;
                    }
                    Ok(n) => {
                        let _ = process_stdout_chunk(
                            &stdout_buf[..n], run_id, writer, parser, app_handle,
                        )
                        .await;
                    }
                }
            }
            result = stderr.read(&mut stderr_buf) => {
                match result {
                    Ok(0) | Err(_) => {}
                    Ok(n) => { let _ = writer.append_raw(&stderr_buf[..n]).await; }
                }
            }
            _ = cancel.cancelled() => {
                tracing::info!(run_id = %run_id, "run cancelled during step-failure wait");
                if let Err(e) = child.kill().await {
                    tracing::warn!(run_id = %run_id, error = %e, "child kill failed during sf wait");
                }
                return StepFailureOutcome::Break { cancelled: true, exit_note: None };
            }
            _ = tokio::time::sleep(remaining) => {
                tracing::info!(run_id = %run_id, "step_failure: auto-Continue after 60 s timeout");
                break 'sf_wait StepFailureChoice::Continue;
            }
        }
    };

    // 3. Apply the chosen action.
    match choice {
        StepFailureChoice::Continue => {
            // Write "\n" to stdin to try to unblock the CLI.
            {
                let mut sg = stdin_arc.lock().await;
                if let Some(ref mut stdin) = *sg {
                    if let Err(e) = stdin.write_all(b"\n").await {
                        tracing::warn!(
                            run_id = %run_id,
                            error = %e,
                            "step_failure Continue: stdin write failed"
                        );
                    }
                }
            }
            // Wait up to 2 s for new stdout output.
            let mut got_output = false;
            let two_s_deadline = tokio::time::Instant::now() + Duration::from_millis(2000);
            'continue_wait: loop {
                let rem = two_s_deadline.saturating_duration_since(tokio::time::Instant::now());
                if rem.is_zero() {
                    break 'continue_wait;
                }
                tokio::select! {
                    result = stdout.read(&mut stdout_buf) => {
                        match result {
                            Ok(0) | Err(_) => break 'continue_wait,
                            Ok(n) => {
                                got_output = true;
                                let _ = process_stdout_chunk(
                                    &stdout_buf[..n], run_id, writer, parser, app_handle,
                                )
                                .await;
                                break 'continue_wait;
                            }
                        }
                    }
                    _ = tokio::time::sleep(rem) => { break 'continue_wait; }
                }
            }

            if got_output {
                tracing::info!(
                    run_id = %run_id,
                    "step_failure Continue: child produced output, resuming"
                );
                StepFailureOutcome::Continue
            } else {
                tracing::info!(
                    run_id = %run_id,
                    "step_failure Continue: no output in 2s, killing and re-invoking"
                );
                if let Err(e) = child.kill().await {
                    tracing::warn!(run_id = %run_id, error = %e, "child kill failed (Continue fallback)");
                }
                if let Some((new_id, new_ctx)) = re_invoke_run(
                    run_id,
                    project_path,
                    launch_input.clone(),
                    cli_path,
                    attached_md_content,
                    sessions,
                )
                .await
                {
                    spawn_reinvoke_task(app_handle.clone(), new_id, new_ctx);
                }
                StepFailureOutcome::Break {
                    cancelled: false,
                    exit_note: None,
                }
            }
        }

        StepFailureChoice::Retry => {
            tracing::info!(run_id = %run_id, "step_failure: Retry");
            if let Err(e) = child.kill().await {
                tracing::warn!(run_id = %run_id, error = %e, "child kill failed (Retry)");
            }
            if let Some((new_id, new_ctx)) = re_invoke_run(
                run_id,
                project_path,
                launch_input.clone(),
                cli_path,
                attached_md_content,
                sessions,
            )
            .await
            {
                spawn_reinvoke_task(app_handle.clone(), new_id, new_ctx);
            }
            StepFailureOutcome::Break {
                cancelled: false,
                exit_note: None,
            }
        }

        StepFailureChoice::Skip => {
            tracing::info!(run_id = %run_id, "step_failure: Skip");
            let mut skip_input = launch_input.clone();
            skip_input.sequence_name = format!(
                "Skip the previous failing step and continue. {}",
                launch_input.sequence_name
            );
            if let Err(e) = child.kill().await {
                tracing::warn!(run_id = %run_id, error = %e, "child kill failed (Skip)");
            }
            if let Some((new_id, new_ctx)) = re_invoke_run(
                run_id,
                project_path,
                skip_input,
                cli_path,
                attached_md_content,
                sessions,
            )
            .await
            {
                spawn_reinvoke_task(app_handle.clone(), new_id, new_ctx);
            }
            StepFailureOutcome::Break {
                cancelled: false,
                exit_note: None,
            }
        }

        StepFailureChoice::Abort => {
            tracing::info!(run_id = %run_id, "step_failure: Abort");
            if let Err(e) = child.kill().await {
                tracing::warn!(run_id = %run_id, error = %e, "child kill failed (Abort)");
            }
            StepFailureOutcome::Break {
                cancelled: false,
                exit_note: Some("Aborted by user".to_string()),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Background I/O helpers (spawn)
// ---------------------------------------------------------------------------

/// Helper: spawn a new `run_io_loop` task from the re-invoke context returned
/// by `re_invoke_run`.  Extracted here so the `Instrument` import does not
/// need to be pulled in at the call sites inside the loop body.
fn spawn_reinvoke_task(app_handle: tauri::AppHandle, new_run_id: String, ctx: RunIoContext) {
    use tracing::Instrument;
    let span = tracing::info_span!("run_session", run_id = %new_run_id);
    tokio::task::spawn(run_io_loop(app_handle, new_run_id, ctx).instrument(span));
}

/// Spawnable background task that:
/// 1. Updates run status to `Running` and emits `run:started`.
/// 2. Streams stdout through `EventParser`, writing events to transcript and
///    emitting `run:event` to the frontend.
/// 3. Appends stderr bytes to `raw.log`.
/// 4. On `StepFailed` event: emits `run:step_failure`, waits up to 60 s for a
///    `StepFailureChoice`, then applies the kill + re-invoke protocol (T4.8
///    option b).
/// 5. On cancellation, kills the child.
/// 6. After the child exits, finalises the run status and emits `run:finished`.
/// 7. Removes the session from the `sessions` map.
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
        launch_input,
        cli_path,
        stdin_arc,
        mut step_failure_rx,
        attached_md_content,
    } = ctx;

    // ── Step 1: mark Running, write meta, emit run:started ──────────────────

    let (project_path, project_id) = {
        let mut run_guard = run.lock().await;
        run_guard.status = RunStatus::Running;
        run_guard.pid = child.id();
        let snapshot = run_guard.clone();
        drop(run_guard);

        let pid = snapshot.pid;
        let project_id = snapshot.project_id.clone();
        let project_path = snapshot.project_path.clone();

        tracing::info!(
            run_id = %run_id,
            project_id = %project_id,
            pid = ?pid,
            "run started"
        );

        if let Err(e) = writer.update_meta(&snapshot).await {
            tracing::warn!(run_id = %run_id, error = %e, "failed to write meta on start");
        }

        (project_path, project_id)
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
    // Set by the Abort handler to populate Run.exit_note in meta.
    let mut step_failure_exit_note: Option<String> = None;

    'io: loop {
        tokio::select! {
            // Read stdout.
            result = stdout.read(&mut stdout_buf) => {
                match result {
                    Ok(0) => {
                        // EOF on stdout — child has closed its write end.
                        break 'io;
                    }
                    Ok(n) => {
                        let chunk = &stdout_buf[..n];
                        let step_failed = process_stdout_chunk(
                            chunk, &run_id, &mut writer, &mut parser, &app_handle,
                        )
                        .await;

                        if step_failed {
                            // ── Step-failure protocol (T4.7 / KB §9 item 3) ──────────
                            let outcome = handle_step_failure(
                                &run_id,
                                &app_handle,
                                &mut stdout,
                                &mut stderr,
                                &mut child,
                                &mut writer,
                                &mut parser,
                                &stdin_arc,
                                &mut step_failure_rx,
                                &cancel,
                                &launch_input,
                                &cli_path,
                                &attached_md_content,
                                sessions.clone(),
                                &project_path,
                            )
                            .await;
                            match outcome {
                                StepFailureOutcome::Continue => {
                                    // Child responded — continue the outer loop normally.
                                }
                                StepFailureOutcome::Break { cancelled: c, exit_note } => {
                                    cancelled = c;
                                    step_failure_exit_note = exit_note;
                                    break 'io;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(run_id = %run_id, error = %e, "stdout read error");
                        break 'io;
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
                break 'io;
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
                            // Ignore step-failure return during drain.
                            let _ = process_stdout_chunk(
                                &stdout_buf[..n], &run_id, &mut writer, &mut parser, &app_handle,
                            )
                            .await;
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
    let exit_code: Option<i32> = exit_status.ok().and_then(|s| s.code());

    // ── Step 5: determine final status ──────────────────────────────────────

    let final_status = if cancelled {
        RunStatus::Stopped
    } else if step_failure_exit_note.is_some() {
        // Abort: always Failed regardless of exit code.
        RunStatus::Failed
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
        if let Some(ref note) = step_failure_exit_note {
            run_guard.exit_note = Some(note.clone());
        }
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
        assert!(
            !validate_run_id("../etc/passwd"),
            "path traversal must be rejected"
        );
        assert!(
            !validate_run_id("/absolute/path"),
            "absolute path must be rejected"
        );
        assert!(
            !validate_run_id("foo/bar"),
            "forward slash must be rejected"
        );
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
        assert!(
            !validate_run_id(" leading"),
            "leading space must be rejected"
        );
        assert!(
            !validate_run_id("trailing "),
            "trailing space must be rejected"
        );
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
        assert!(validate_run_id("a"), "single-char id must be accepted");
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
        assert!(!validate_run_id("\0"), "lone null byte must be rejected");
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

    // ── T4.7: StepFailureChoice serialization ────────────────────────────────

    /// All four `StepFailureChoice` variants must round-trip through JSON.
    /// This ensures the IPC layer can deserialize choices sent by the frontend.
    #[test]
    fn step_failure_choice_serializes_and_deserializes() {
        let cases = [
            (StepFailureChoice::Continue, "\"Continue\""),
            (StepFailureChoice::Retry, "\"Retry\""),
            (StepFailureChoice::Skip, "\"Skip\""),
            (StepFailureChoice::Abort, "\"Abort\""),
        ];
        for (variant, expected_json) in cases {
            let serialized =
                serde_json::to_string(&variant).expect("StepFailureChoice must serialize");
            assert_eq!(serialized, expected_json, "variant serialization mismatch");
            let deserialized: StepFailureChoice =
                serde_json::from_str(&serialized).expect("StepFailureChoice must deserialize");
            // Re-serialize to compare (enum doesn't implement PartialEq).
            let re_serialized = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(re_serialized, expected_json, "round-trip failed");
        }
    }

    /// `Run.exit_note` and `Run.retry_of` are optional and default to `None`
    /// when absent from a deserialized JSON object (backward-compat guard).
    #[test]
    fn run_exit_note_and_retry_of_default_to_none_when_absent() {
        // Simulate an old meta.json that does not have the new fields.
        let json = r#"{
            "id": "test-run",
            "project_id": "proj-1",
            "project_path": "/tmp/proj",
            "sequence_name": "my-seq",
            "attached_md_path": null,
            "started_at": "2026-01-01T00:00:00Z",
            "ended_at": null,
            "status": "Running",
            "exit_code": null,
            "pid": null,
            "note": null
        }"#;
        let run: Run =
            serde_json::from_str(json).expect("must deserialize without exit_note / retry_of");
        assert!(
            run.exit_note.is_none(),
            "exit_note must default to None when absent"
        );
        assert!(
            run.retry_of.is_none(),
            "retry_of must default to None when absent"
        );
    }

    /// `Run.exit_note` and `Run.retry_of` are serialized when `Some` and
    /// omitted from the JSON output when `None` (space-saving).
    #[test]
    fn run_exit_note_and_retry_of_serialize_correctly() {
        let run = Run {
            id: "r1".to_string(),
            project_id: "p1".to_string(),
            project_path: PathBuf::from("/tmp"),
            sequence_name: "s".to_string(),
            attached_md_path: None,
            started_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
            ended_at: None,
            status: RunStatus::Failed,
            exit_code: Some(1),
            pid: None,
            note: None,
            exit_note: Some("Aborted by user".to_string()),
            retry_of: Some("original-run-id".to_string()),
        };
        let json = serde_json::to_string(&run).expect("serialize");
        assert!(
            json.contains("\"exit_note\":\"Aborted by user\""),
            "exit_note must be serialized when Some; got: {json}"
        );
        assert!(
            json.contains("\"retry_of\":\"original-run-id\""),
            "retry_of must be serialized when Some; got: {json}"
        );

        // When both are None, they must NOT appear in the JSON (skip_serializing_if).
        let run_no_extras = Run {
            id: "r2".to_string(),
            project_id: "p1".to_string(),
            project_path: PathBuf::from("/tmp"),
            sequence_name: "s".to_string(),
            attached_md_path: None,
            started_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
            ended_at: None,
            status: RunStatus::Running,
            exit_code: None,
            pid: None,
            note: None,
            exit_note: None,
            retry_of: None,
        };
        let json2 = serde_json::to_string(&run_no_extras).expect("serialize");
        assert!(
            !json2.contains("exit_note"),
            "exit_note must be omitted when None; got: {json2}"
        );
        assert!(
            !json2.contains("retry_of"),
            "retry_of must be omitted when None; got: {json2}"
        );
    }

    /// Skip prefix is correctly prepended to `sequence_name`.
    #[test]
    fn skip_prefix_format_is_correct() {
        let original = "my task";
        let prefixed = format!("Skip the previous failing step and continue. {}", original);
        assert_eq!(
            prefixed,
            "Skip the previous failing step and continue. my task"
        );
    }
}
