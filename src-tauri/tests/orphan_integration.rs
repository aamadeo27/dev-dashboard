/// Integration tests for the OrphanReaper (T4.5).
///
/// These tests exercise `runs::reap_orphans` end-to-end against real
/// temporary directories and a real filesystem.  They complement the 11
/// inline unit tests in `src/runs/orphan.rs` by covering:
///
///   - Filesystem round-trip: meta.json is actually written/overwritten.
///   - Atomic write: the `.orphan.tmp` sentinel file is gone after a
///     successful mark.
///   - Live-process paths: spawn a real process, verify it is killed (or
///     conservatively NOT killed) by inspecting the process table after the
///     sweep.
///
/// # What is NOT tested here
///
/// - `maybe_kill_pid` low-level logic — covered by inline unit tests 8/9.
/// - sysinfo refresh failure path — covered inline.
/// - Tauri command wiring — not applicable (orphan reaper has no IPC command).
///
/// # How to run
///
/// ```sh
/// cargo test --manifest-path src-tauri/Cargo.toml --test orphan_integration
/// ```

use dev_dashboard_lib::runs::{reap_orphans, Run, RunStatus};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a minimal `meta.json` with the given status and optional PID into
/// `<run_dir>/meta.json`.  Creates parent dirs.
fn write_meta(run_dir: &Path, id: &str, status: RunStatus, pid: Option<u32>) {
    std::fs::create_dir_all(run_dir).expect("create run_dir");
    let run = Run {
        id: id.to_string(),
        project_id: "test-proj".to_string(),
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
    let json = serde_json::to_string_pretty(&run).expect("serialize run");
    std::fs::write(run_dir.join("meta.json"), json).expect("write meta.json");
}

/// Read `meta.json` back from disk and deserialize.
fn read_meta(run_dir: &Path) -> Run {
    let bytes = std::fs::read(run_dir.join("meta.json")).expect("read meta.json");
    serde_json::from_slice(&bytes).expect("parse meta.json")
}

/// Resolve the absolute path to a long-running process we can safely spawn in
/// tests.  Returns `(exe_path, args)`.
///
/// Requirements for the chosen executable:
///   1. Always present on the target OS.
///   2. Runs without user interaction and does not read stdin.
///   3. Stays alive long enough (>> the reaper sweep duration) so `try_wait`
///      still sees it running when we check (unless the reaper killed it).
///   4. Its exe path is readable by sysinfo without elevated privileges (needed
///      for the conservative exe-matching check in `maybe_kill_pid`).
///
/// - Windows: `cmd.exe /c timeout /t 60 /nobreak` — `timeout.exe` is a child
///   spawned by `cmd.exe`.  We record `cmd.exe`'s PID and use `cmd.exe`'s path
///   as `cli_path`.  `timeout /nobreak` ignores keystrokes so no stdin needed.
/// - Unix:    `/bin/sleep 60`
#[cfg(target_os = "windows")]
fn long_running_cmd() -> (PathBuf, Vec<String>) {
    // cmd.exe: guaranteed present; sysinfo can read its exe without elevation.
    let exe = PathBuf::from(r"C:\Windows\System32\cmd.exe");
    (exe, vec!["/c".to_string(), "timeout".to_string(), "/t".to_string(), "60".to_string(), "/nobreak".to_string()])
}

#[cfg(not(target_os = "windows"))]
fn long_running_cmd() -> (PathBuf, Vec<String>) {
    (PathBuf::from("/bin/sleep"), vec!["60".to_string()])
}

// ---------------------------------------------------------------------------
// IT-OR-01: pending run + dead PID → meta.json updated to Failed
// ---------------------------------------------------------------------------

/// A `Pending` run whose recorded PID is not in the live process table must be
/// marked `Failed` with the canonical orphan note and `ended_at` set.
#[tokio::test]
async fn it_or_01_pending_dead_pid_marked_failed() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-pending");

    // PID 9_999_999 is astronomically unlikely to be alive.
    write_meta(&run_dir, "run-pending", RunStatus::Pending, Some(9_999_999));

    let fake_cli = PathBuf::from("/fake/claude");
    reap_orphans(&[project_path], Some(&fake_cli)).await;

    let meta = read_meta(&run_dir);
    assert!(
        matches!(meta.status, RunStatus::Failed),
        "Pending run with dead PID must be marked Failed"
    );
    assert_eq!(
        meta.note.as_deref(),
        Some("Terminated (app restarted)"),
        "note must be the orphan sentinel"
    );
    assert!(
        meta.ended_at.is_some(),
        "ended_at must be populated after orphan mark"
    );
}

// ---------------------------------------------------------------------------
// IT-OR-02: running run + dead PID → meta.json updated to Failed
// ---------------------------------------------------------------------------

/// A `Running` run with a dead PID must be marked `Failed`.
#[tokio::test]
async fn it_or_02_running_dead_pid_marked_failed() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-running");

    write_meta(&run_dir, "run-running", RunStatus::Running, Some(9_999_999));

    reap_orphans(&[project_path], None).await;

    let meta = read_meta(&run_dir);
    assert!(
        matches!(meta.status, RunStatus::Failed),
        "Running run with dead PID must be marked Failed"
    );
    assert_eq!(
        meta.note.as_deref(),
        Some("Terminated (app restarted)"),
        "note must be the orphan sentinel"
    );
    assert!(meta.ended_at.is_some(), "ended_at must be set");
}

// ---------------------------------------------------------------------------
// IT-OR-03: live run matching CLI exe → process killed, meta.json Failed
// ---------------------------------------------------------------------------

/// Spawn a real process, record its PID in a `Running` meta.json, and set the
/// cli_path to the spawned exe.  After `reap_orphans`, the process must no
/// longer be alive and the meta must be Failed.
///
/// The test gracefully skips if the helper exe is not found on this platform.
#[tokio::test]
async fn it_or_03_live_matching_process_killed_and_meta_failed() {
    let (exe_path, args) = long_running_cmd();

    if !exe_path.exists() {
        // Gracefully skip if the helper binary is not available on this image.
        eprintln!("SKIP it_or_03: {:?} not found", exe_path);
        return;
    }

    // Spawn the helper process.
    let mut child = std::process::Command::new(&exe_path)
        .args(&args)
        // Suppress I/O so the test console stays clean.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn long-running helper");
    let pid = child.id();

    // Set up the meta.json with the live PID.
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-live");
    write_meta(&run_dir, "run-live", RunStatus::Running, Some(pid));

    // Call the reaper with cli_path == the spawned exe (so kill is permitted).
    reap_orphans(&[project_path], Some(&exe_path)).await;

    // Give the OS a moment to reap the child.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify the process is no longer alive by attempting wait with WNOHANG.
    // `try_wait` returns Ok(Some(_)) if exited, Ok(None) if still running.
    let exit_status = child.try_wait().expect("try_wait");
    assert!(
        exit_status.is_some(),
        "process (PID {pid}) should have been killed by the orphan reaper but is still running"
    );

    // meta.json must be Failed.
    let meta = read_meta(&run_dir);
    assert!(
        matches!(meta.status, RunStatus::Failed),
        "meta.json must be Failed after reaper kills the process"
    );
    assert_eq!(
        meta.note.as_deref(),
        Some("Terminated (app restarted)"),
    );
}

// ---------------------------------------------------------------------------
// IT-OR-04: conservative — live PID with wrong exe → NOT killed, run still Failed
// ---------------------------------------------------------------------------

/// Spawn a real process, record its PID, but pass a *different* cli_path that
/// does NOT match the spawned exe.  The reaper must NOT kill the process (it
/// belongs to an unrelated program) but must still mark the run Failed.
#[tokio::test]
async fn it_or_04_live_mismatched_exe_not_killed_run_marked_failed() {
    let (exe_path, args) = long_running_cmd();

    if !exe_path.exists() {
        eprintln!("SKIP it_or_04: {:?} not found", exe_path);
        return;
    }

    let mut child = std::process::Command::new(&exe_path)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn long-running helper");
    let pid = child.id();

    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-mismatch");
    write_meta(&run_dir, "run-mismatch", RunStatus::Running, Some(pid));

    // Use a cli_path that does NOT match the spawned exe.
    let wrong_cli = PathBuf::from("/totally/different/path/claude");
    reap_orphans(&[project_path], Some(&wrong_cli)).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Process must still be alive (conservative rule: exe mismatch → no kill).
    let exit_status = child.try_wait().expect("try_wait");
    assert!(
        exit_status.is_none(),
        "process (PID {pid}) must NOT be killed when exe path does not match cli_path"
    );

    // meta.json must still be Failed (status update happens regardless of kill).
    let meta = read_meta(&run_dir);
    assert!(
        matches!(meta.status, RunStatus::Failed),
        "run must be marked Failed even when the process is not killed (conservative)"
    );
    assert_eq!(
        meta.note.as_deref(),
        Some("Terminated (app restarted)"),
    );

    // Clean up: kill the child so it doesn't linger after the test.
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// IT-OR-05: completed/stopped runs → untouched on disk
// ---------------------------------------------------------------------------

/// `Completed` and `Stopped` runs must not be mutated by the reaper.
/// We verify both status and the absence of a note (which was None at write
/// time) to confirm the file was not rewritten.
#[tokio::test]
async fn it_or_05_completed_stopped_runs_untouched() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let runs_dir = project_path.join(".claude").join("runs");

    let completed_dir = runs_dir.join("run-completed");
    let stopped_dir = runs_dir.join("run-stopped");
    write_meta(&completed_dir, "run-completed", RunStatus::Completed, None);
    write_meta(&stopped_dir, "run-stopped", RunStatus::Stopped, None);

    reap_orphans(&[project_path], None).await;

    let comp = read_meta(&completed_dir);
    let stop = read_meta(&stopped_dir);

    assert!(
        matches!(comp.status, RunStatus::Completed),
        "Completed run must not be changed by orphan reaper"
    );
    assert!(
        comp.note.is_none(),
        "Completed run note must not be set by orphan reaper"
    );
    assert!(
        comp.ended_at.is_none(),
        "Completed run ended_at must not be set by orphan reaper (was None at write time)"
    );

    assert!(
        matches!(stop.status, RunStatus::Stopped),
        "Stopped run must not be changed by orphan reaper"
    );
    assert!(
        stop.note.is_none(),
        "Stopped run note must not be set by orphan reaper"
    );
}

// ---------------------------------------------------------------------------
// IT-OR-06: missing runs dir → no panic, returns normally
// ---------------------------------------------------------------------------

/// When `.claude/runs/` does not exist the reaper must return without panic.
/// This is the normal path for freshly-registered projects.
#[tokio::test]
async fn it_or_06_missing_runs_dir_no_panic() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    // Do NOT create .claude/runs/.

    // Must return without panic.
    reap_orphans(&[project_path], None).await;
}

// ---------------------------------------------------------------------------
// IT-OR-07: atomic write — meta.json.orphan.tmp cleaned up after successful mark
// ---------------------------------------------------------------------------

/// After a successful orphan mark the intermediate tmp file used by the atomic
/// write must not be present on disk.
#[tokio::test]
async fn it_or_07_orphan_tmp_file_cleaned_up_after_mark() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-atomic");
    write_meta(&run_dir, "run-atomic", RunStatus::Pending, None);

    reap_orphans(&[project_path], None).await;

    // meta.json must exist and be Failed.
    let meta = read_meta(&run_dir);
    assert!(matches!(meta.status, RunStatus::Failed));

    // meta.json.orphan.tmp must NOT be present.
    let tmp_path = run_dir.join("meta.json.orphan.tmp");
    assert!(
        !tmp_path.exists(),
        "meta.json.orphan.tmp must be renamed away after a successful atomic write"
    );
}

// ---------------------------------------------------------------------------
// IT-OR-08: multiple projects, all orphaned runs marked Failed
// ---------------------------------------------------------------------------

/// Two separate project dirs, each containing one in-flight run.  Both must
/// be marked Failed by a single reaper sweep.
#[tokio::test]
async fn it_or_08_multiple_projects_all_orphans_marked() {
    let tmp_a = TempDir::new().expect("tempdir A");
    let tmp_b = TempDir::new().expect("tempdir B");

    let proj_a = tmp_a.path().to_path_buf();
    let proj_b = tmp_b.path().to_path_buf();

    let run_dir_a = proj_a.join(".claude").join("runs").join("run-a");
    let run_dir_b = proj_b.join(".claude").join("runs").join("run-b");

    write_meta(&run_dir_a, "run-a", RunStatus::Running, None);
    write_meta(&run_dir_b, "run-b", RunStatus::Pending, None);

    reap_orphans(&[proj_a, proj_b], None).await;

    let meta_a = read_meta(&run_dir_a);
    let meta_b = read_meta(&run_dir_b);

    assert!(matches!(meta_a.status, RunStatus::Failed), "project A run must be Failed");
    assert!(matches!(meta_b.status, RunStatus::Failed), "project B run must be Failed");

    assert_eq!(meta_a.note.as_deref(), Some("Terminated (app restarted)"));
    assert_eq!(meta_b.note.as_deref(), Some("Terminated (app restarted)"));
}

// ---------------------------------------------------------------------------
// IT-OR-09: mixed statuses in one project — only Pending/Running marked
// ---------------------------------------------------------------------------

/// A project containing all five statuses: Pending and Running must become
/// Failed; the other three must be untouched.
#[tokio::test]
async fn it_or_09_mixed_statuses_only_inflight_marked() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let runs_dir = project_path.join(".claude").join("runs");

    let dirs = [
        ("r-pending",   RunStatus::Pending),
        ("r-running",   RunStatus::Running),
        ("r-failed",    RunStatus::Failed),
        ("r-completed", RunStatus::Completed),
        ("r-stopped",   RunStatus::Stopped),
    ];

    for (name, status) in &dirs {
        write_meta(&runs_dir.join(name), name, status.clone(), None);
    }

    reap_orphans(&[project_path], None).await;

    assert!(matches!(read_meta(&runs_dir.join("r-pending")).status,   RunStatus::Failed));
    assert!(matches!(read_meta(&runs_dir.join("r-running")).status,   RunStatus::Failed));
    assert!(matches!(read_meta(&runs_dir.join("r-failed")).status,    RunStatus::Failed));   // was already Failed
    assert!(matches!(read_meta(&runs_dir.join("r-completed")).status, RunStatus::Completed));
    assert!(matches!(read_meta(&runs_dir.join("r-stopped")).status,   RunStatus::Stopped));

    // Confirm the pre-existing Failed run was NOT rewritten (its note is still None).
    assert!(
        read_meta(&runs_dir.join("r-failed")).note.is_none(),
        "pre-existing Failed run must not be rewritten by the orphan reaper"
    );
}

// ---------------------------------------------------------------------------
// IT-OR-10: cli_path = None → kill skipped, run still marked Failed
// ---------------------------------------------------------------------------

/// When `claude_cli_path` is not configured the reaper must skip the kill
/// step (conservative: can't verify exe match) but must still mark the run
/// Failed.  This tests the first-launch case.
#[tokio::test]
async fn it_or_10_no_cli_path_kill_skipped_run_still_failed() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-nocli");

    // Use a dead PID — we only care that the run is marked Failed and that
    // passing None for cli_path doesn't cause a panic or prevent the mark.
    write_meta(&run_dir, "run-nocli", RunStatus::Running, Some(9_999_999));

    reap_orphans(&[project_path], None).await;

    let meta = read_meta(&run_dir);
    assert!(
        matches!(meta.status, RunStatus::Failed),
        "run must be marked Failed even when cli_path is None"
    );
    assert_eq!(
        meta.note.as_deref(),
        Some("Terminated (app restarted)"),
    );
}
