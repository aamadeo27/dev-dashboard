// OrphanReaper — startup PID cleanup for stale pending/running runs.
//
// On app startup, scans all registered projects' `.claude/runs/*/meta.json`
// files. For any run whose status is `Pending` or `Running`:
//   1. If the recorded PID is alive AND its executable path matches the
//      configured claude CLI path → send SIGTERM / TerminateProcess to kill it.
//   2. Regardless of kill success, overwrite `meta.json` to mark the run
//      `Failed` with `note = "Terminated (app restarted)"`.
//
// The conservative rule: a PID that is alive but has a *different* exe path
// is NOT killed (another unrelated process has reused that PID).

use std::path::{Path, PathBuf};

use chrono::Utc;
use sysinfo::{Pid, System};

use super::{Run, RunStatus};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the orphan-reaper sweep across all registered project paths.
///
/// # Arguments
/// * `project_paths` — canonicalized paths of all registered projects.
/// * `claude_cli_path` — the configured claude CLI absolute path, or `None`
///   if not yet configured.  When `None`, PID killing is skipped (we cannot
///   compare the exe path) but orphaned meta files are still marked `Failed`.
///
/// # Behaviour
/// This function is intentionally infallible at the top level: I/O errors on
/// individual run directories are logged and skipped rather than aborting the
/// whole sweep.  The app must start even if some project dirs are missing.
pub async fn run(project_paths: &[PathBuf], claude_cli_path: Option<&Path>) {
    tracing::info!(
        component = "orphan_reaper",
        project_count = project_paths.len(),
        cli_configured = claude_cli_path.is_some(),
        "orphan reaper started"
    );

    // Load the sysinfo process table once for the whole sweep.
    // refresh_processes() populates exe paths (needed for the conservative
    // PID match).  We do this on the calling (async) thread via spawn_blocking
    // so we don't block the Tokio executor during the syscall.
    let system = tokio::task::spawn_blocking(|| {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys
    })
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(
            component = "orphan_reaper",
            error = %e,
            "failed to refresh process list; will skip PID killing"
        );
        System::new()
    });

    let mut killed = 0u32;
    let mut marked = 0u32;

    for project_path in project_paths {
        let runs_dir = project_path.join(".claude").join("runs");

        // If the runs directory doesn't exist, skip silently (normal for new projects).
        let mut entries = match tokio::fs::read_dir(&runs_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                tracing::warn!(
                    component = "orphan_reaper",
                    path = %runs_dir.display(),
                    error = %e,
                    "failed to read runs directory; skipping project"
                );
                continue;
            }
        };

        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(
                        component = "orphan_reaper",
                        error = %e,
                        "error iterating runs directory entry; skipping"
                    );
                    continue;
                }
            };

            let run_dir = entry.path();

            // Only process directories.
            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => {}
                _ => continue,
            }

            let meta_path = run_dir.join("meta.json");

            // Read and deserialize meta.json.
            let run = match read_meta(&meta_path).await {
                Some(r) => r,
                None => continue,
            };

            // Only care about runs that were still in-flight.
            if !matches!(run.status, RunStatus::Pending | RunStatus::Running) {
                continue;
            }

            tracing::info!(
                component = "orphan_reaper",
                run_id = %run.id,
                project_id = %run.project_id,
                status = ?run.status,
                pid = ?run.pid,
                "found orphaned run"
            );

            // Attempt to kill the child process if PID is known and exe matches.
            if let Some(pid) = run.pid {
                let was_killed = maybe_kill_pid(pid, claude_cli_path, &system, &run.id);
                if was_killed {
                    killed += 1;
                }
            }

            // Mark the run failed regardless of kill outcome.
            mark_failed(&meta_path, run).await;
            marked += 1;
        }
    }

    tracing::info!(
        component = "orphan_reaper",
        killed,
        marked,
        "orphan reaper finished"
    );
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Read and deserialize `meta.json`, returning `None` on any I/O or parse
/// error (errors are logged at warn level).
async fn read_meta(meta_path: &Path) -> Option<Run> {
    let bytes = match tokio::fs::read(meta_path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(
                component = "orphan_reaper",
                path = %meta_path.display(),
                error = %e,
                "failed to read meta.json; skipping"
            );
            return None;
        }
    };

    match serde_json::from_slice::<Run>(&bytes) {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!(
                component = "orphan_reaper",
                path = %meta_path.display(),
                error = %e,
                "failed to parse meta.json; skipping"
            );
            None
        }
    }
}

/// Conditionally kill `pid` if it is alive AND its exe path matches
/// `claude_cli_path`.  Returns `true` if a kill signal was sent.
///
/// Conservative: if `claude_cli_path` is `None` or the exe does not match,
/// the PID is left alone.
fn maybe_kill_pid(pid: u32, claude_cli_path: Option<&Path>, system: &System, run_id: &str) -> bool {
    let sysinfo_pid = Pid::from_u32(pid);

    let process = match system.process(sysinfo_pid) {
        Some(p) => p,
        None => {
            tracing::info!(
                component = "orphan_reaper",
                run_id = %run_id,
                pid,
                "PID no longer alive; skipping kill"
            );
            return false;
        }
    };

    // Resolve the configured CLI path to compare against the process exe.
    let expected_exe = match claude_cli_path {
        Some(p) => p,
        None => {
            tracing::info!(
                component = "orphan_reaper",
                run_id = %run_id,
                pid,
                "claude CLI path not configured; skipping kill (conservative)"
            );
            return false;
        }
    };

    // Get the process exe path; if unavailable we skip (conservative).
    let process_exe = match process.exe() {
        Some(e) => e,
        None => {
            tracing::info!(
                component = "orphan_reaper",
                run_id = %run_id,
                pid,
                "process exe path unavailable; skipping kill (conservative)"
            );
            return false;
        }
    };

    // Normalize both paths to string for comparison (handles different
    // separator styles on Windows).
    let expected_str = expected_exe.to_string_lossy().to_lowercase();
    let process_str = process_exe.to_string_lossy().to_lowercase();

    if expected_str != process_str {
        tracing::info!(
            component = "orphan_reaper",
            run_id = %run_id,
            pid,
            expected_exe = %expected_exe.display(),
            actual_exe = %process_exe.display(),
            "exe mismatch — PID belongs to a different program; skipping kill (conservative)"
        );
        return false;
    }

    // Exe matches — kill it.
    let kill_sent = process.kill();
    tracing::info!(
        component = "orphan_reaper",
        run_id = %run_id,
        pid,
        exe = %process_exe.display(),
        kill_sent,
        "sent kill signal to orphaned claude process"
    );
    kill_sent
}

/// Overwrite `meta.json` to mark `run` as `Failed` with the orphan note.
/// Errors are logged at warn level and do not propagate (startup must continue).
async fn mark_failed(meta_path: &Path, mut run: Run) {
    run.status = RunStatus::Failed;
    run.ended_at = Some(Utc::now());
    run.note = Some("Terminated (app restarted)".to_string());

    let pretty = match serde_json::to_string_pretty(&run) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(
                component = "orphan_reaper",
                run_id = %run.id,
                error = %e,
                "failed to serialize run for orphan mark; skipping"
            );
            return;
        }
    };

    // Atomic write: tmp then rename.
    let run_dir = match meta_path.parent() {
        Some(d) => d,
        None => {
            tracing::warn!(
                component = "orphan_reaper",
                run_id = %run.id,
                "meta_path has no parent; skipping"
            );
            return;
        }
    };
    let tmp_path = run_dir.join("meta.json.orphan.tmp");

    if let Err(e) = tokio::fs::write(&tmp_path, pretty.as_bytes()).await {
        tracing::warn!(
            component = "orphan_reaper",
            run_id = %run.id,
            path = %tmp_path.display(),
            error = %e,
            "failed to write orphan meta tmp file; skipping"
        );
        return;
    }

    if let Err(e) = tokio::fs::rename(&tmp_path, meta_path).await {
        tracing::warn!(
            component = "orphan_reaper",
            run_id = %run.id,
            error = %e,
            "failed to rename orphan meta tmp to meta.json; skipping"
        );
        // Best-effort cleanup of the tmp file.
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return;
    }

    tracing::info!(
        component = "orphan_reaper",
        run_id = %run.id,
        "run marked failed (orphan reaper)"
    );
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Write a minimal `meta.json` with the given status and optional PID
    /// into `<run_dir>/meta.json`.  Creates parent dirs.
    pub(crate) fn write_meta(run_dir: &Path, id: &str, status: RunStatus, pid: Option<u32>) {
        std::fs::create_dir_all(run_dir).expect("create run_dir");
        let run = Run {
            id: id.to_string(),
            project_id: "proj-1".to_string(),
            project_path: PathBuf::from("/fake/project"),
            sequence_name: "test-seq".to_string(),
            attached_md_path: None,
            started_at: chrono::Utc::now(),
            ended_at: None,
            status,
            exit_code: None,
            pid,
            note: None,
        };
        let json = serde_json::to_string_pretty(&run).expect("serialize");
        std::fs::write(run_dir.join("meta.json"), json).expect("write meta.json");
    }

    /// Read `meta.json` back from disk and deserialize.
    pub(crate) fn read_meta_sync(run_dir: &Path) -> Run {
        let bytes = std::fs::read(run_dir.join("meta.json")).expect("read meta.json");
        serde_json::from_slice(&bytes).expect("parse meta.json")
    }

    // ── Test 1: completed/stopped runs are left untouched ────────────────────

    #[tokio::test]
    async fn completed_and_stopped_runs_are_not_touched() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // Two runs that must NOT be touched.
        let completed_dir = runs_dir.join("run-completed");
        let stopped_dir = runs_dir.join("run-stopped");
        write_meta(&completed_dir, "run-completed", RunStatus::Completed, None);
        write_meta(&stopped_dir, "run-stopped", RunStatus::Stopped, None);

        run(&[project_path], None).await;

        // Status must be unchanged.
        let comp = read_meta_sync(&completed_dir);
        let stop = read_meta_sync(&stopped_dir);
        assert!(
            matches!(comp.status, RunStatus::Completed),
            "Completed run must not be changed"
        );
        assert!(
            matches!(stop.status, RunStatus::Stopped),
            "Stopped run must not be changed"
        );
    }

    // ── Test 2: pending run with no PID is marked Failed ────────────────────

    #[tokio::test]
    async fn pending_run_no_pid_is_marked_failed() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let run_dir = project_path.join(".claude").join("runs").join("run-pending");
        write_meta(&run_dir, "run-pending", RunStatus::Pending, None);

        run(&[project_path], None).await;

        let meta = read_meta_sync(&run_dir);
        assert!(
            matches!(meta.status, RunStatus::Failed),
            "Pending run must be marked Failed"
        );
        assert_eq!(
            meta.note.as_deref(),
            Some("Terminated (app restarted)"),
            "note must be the orphan sentinel"
        );
        assert!(
            meta.ended_at.is_some(),
            "ended_at must be set after orphan mark"
        );
    }

    // ── Test 3: running run with no PID is marked Failed ────────────────────

    #[tokio::test]
    async fn running_run_no_pid_is_marked_failed() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let run_dir = project_path.join(".claude").join("runs").join("run-running");
        write_meta(&run_dir, "run-running", RunStatus::Running, None);

        run(&[project_path], None).await;

        let meta = read_meta_sync(&run_dir);
        assert!(
            matches!(meta.status, RunStatus::Failed),
            "Running run must be marked Failed"
        );
        assert_eq!(meta.note.as_deref(), Some("Terminated (app restarted)"));
    }

    // ── Test 4: runs dir does not exist → no panic ───────────────────────────

    #[tokio::test]
    async fn missing_runs_dir_does_not_panic() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        // Intentionally do NOT create .claude/runs/.

        // Must not panic.
        run(&[project_path], None).await;
    }

    // ── Test 5: empty project list → no panic ────────────────────────────────

    #[tokio::test]
    async fn empty_project_list_does_not_panic() {
        run(&[], None).await;
    }

    // ── Test 6: mixed statuses — only pending/running are touched ────────────

    #[tokio::test]
    async fn only_inflight_statuses_are_marked_failed() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        let pending_dir = runs_dir.join("r-pending");
        let running_dir = runs_dir.join("r-running");
        let failed_dir = runs_dir.join("r-failed");
        let completed_dir = runs_dir.join("r-completed");
        let stopped_dir = runs_dir.join("r-stopped");

        write_meta(&pending_dir, "r-pending", RunStatus::Pending, None);
        write_meta(&running_dir, "r-running", RunStatus::Running, None);
        write_meta(&failed_dir, "r-failed", RunStatus::Failed, None);
        write_meta(&completed_dir, "r-completed", RunStatus::Completed, None);
        write_meta(&stopped_dir, "r-stopped", RunStatus::Stopped, None);

        run(&[project_path], None).await;

        // Pending → Failed.
        assert!(matches!(read_meta_sync(&pending_dir).status, RunStatus::Failed));
        // Running → Failed.
        assert!(matches!(read_meta_sync(&running_dir).status, RunStatus::Failed));
        // Others unchanged.
        assert!(matches!(read_meta_sync(&failed_dir).status, RunStatus::Failed));
        assert!(matches!(read_meta_sync(&completed_dir).status, RunStatus::Completed));
        assert!(matches!(read_meta_sync(&stopped_dir).status, RunStatus::Stopped));
    }

    // ── Test 7: corrupt meta.json is skipped without panicking ───────────────

    #[tokio::test]
    async fn corrupt_meta_json_is_skipped() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let run_dir = project_path.join(".claude").join("runs").join("run-corrupt");
        std::fs::create_dir_all(&run_dir).expect("create run_dir");
        std::fs::write(run_dir.join("meta.json"), b"{ not valid json !!! }").expect("write corrupt");

        // Must not panic.
        run(&[project_path], None).await;
    }

    // ── Test 8: maybe_kill_pid — PID not found in system → no kill ───────────
    //
    // We cannot spawn a real process in a unit test and control its exe path,
    // but we can test `maybe_kill_pid` directly with an empty `System` that
    // has zero processes, simulating "PID not found".
    #[test]
    fn maybe_kill_pid_returns_false_when_pid_not_in_system() {
        let system = System::new(); // empty — no processes refreshed
        let fake_cli = PathBuf::from("/usr/local/bin/claude");

        // PID 9_999_999 is very unlikely to be a real process; the system was
        // not refreshed so it definitely won't appear.
        let result = maybe_kill_pid(9_999_999, Some(&fake_cli), &system, "test-run");
        assert!(!result, "must return false when PID is not in the system snapshot");
    }

    // ── Test 9: maybe_kill_pid — None cli_path → false ───────────────────────

    #[test]
    fn maybe_kill_pid_returns_false_when_cli_path_is_none() {
        let system = System::new();
        let result = maybe_kill_pid(1, None, &system, "test-run");
        assert!(!result, "must return false when CLI path is not configured");
    }

    // ── Test 10: multiple projects, each with orphaned runs ──────────────────

    #[tokio::test]
    async fn multiple_projects_all_orphans_marked() {
        let tmp_a = TempDir::new().expect("tempdir A");
        let tmp_b = TempDir::new().expect("tempdir B");

        let proj_a = tmp_a.path().to_path_buf();
        let proj_b = tmp_b.path().to_path_buf();

        let run_dir_a = proj_a.join(".claude").join("runs").join("run-a");
        let run_dir_b = proj_b.join(".claude").join("runs").join("run-b");
        write_meta(&run_dir_a, "run-a", RunStatus::Running, None);
        write_meta(&run_dir_b, "run-b", RunStatus::Pending, None);

        run(&[proj_a, proj_b], None).await;

        assert!(matches!(read_meta_sync(&run_dir_a).status, RunStatus::Failed));
        assert!(matches!(read_meta_sync(&run_dir_b).status, RunStatus::Failed));
        assert_eq!(
            read_meta_sync(&run_dir_a).note.as_deref(),
            Some("Terminated (app restarted)")
        );
        assert_eq!(
            read_meta_sync(&run_dir_b).note.as_deref(),
            Some("Terminated (app restarted)")
        );
    }

    // ── Test 11: non-directory entries in runs dir are silently skipped ───────

    #[tokio::test]
    async fn non_dir_entries_in_runs_dir_are_skipped() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");
        std::fs::create_dir_all(&runs_dir).expect("create runs_dir");

        // Place a plain file (not a directory) in the runs dir.
        std::fs::write(runs_dir.join("stray_file.txt"), b"not a run dir")
            .expect("write stray file");

        // Also place a legitimate run.
        let run_dir = runs_dir.join("run-ok");
        write_meta(&run_dir, "run-ok", RunStatus::Pending, None);

        run(&[project_path], None).await;

        // The stray file causes no panic; the real run is still processed.
        assert!(matches!(read_meta_sync(&run_dir).status, RunStatus::Failed));
    }
}
