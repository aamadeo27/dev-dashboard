/// Integration tests for the RetentionPruner (T4.6).
///
/// These tests exercise `runs::prune_runs` end-to-end against real temporary
/// directories and a real filesystem.  They complement the 10 inline unit tests
/// in `src/runs/retention.rs` by providing an additional integration-level
/// harness that mirrors the orphan integration test style.
///
/// # What is NOT tested here
///
/// - `measure_dir_size` low-level logic — covered by inline unit tests.
/// - `start` background timer loop — no Tauri AppHandle available in tests.
/// - Tauri command wiring — RetentionPruner has no IPC command.
///
/// # How to run
///
/// ```sh
/// cargo test --manifest-path src-tauri/Cargo.toml --test retention_integration
/// ```
use dev_dashboard_lib::runs::{prune_runs, Run, RunStatus};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a `meta.json` with the given status and `started_at` offset in days
/// from now (positive = days ago).  Creates parent dirs.
///
/// Note: mirrors the inline helpers in retention.rs unit tests. Kept separate
/// because integration tests live in a different crate and cannot access
/// `#[cfg(test)]` helpers from the library crate.
fn write_meta(run_dir: &Path, id: &str, status: RunStatus, days_ago: i64) {
    std::fs::create_dir_all(run_dir).expect("create run_dir");
    let started_at = chrono::Utc::now() - chrono::Duration::days(days_ago);
    let run = Run {
        id: id.to_string(),
        project_id: "test-proj".to_string(),
        project_path: PathBuf::from("/fake/project"),
        sequence_name: "test-seq".to_string(),
        attached_md_path: None,
        started_at,
        ended_at: Some(started_at + chrono::Duration::minutes(5)),
        status,
        exit_code: Some(0),
        pid: None,
        note: None,
        exit_note: None,
        retry_of: None,
    };
    let json = serde_json::to_string_pretty(&run).expect("serialize run");
    std::fs::write(run_dir.join("meta.json"), json).expect("write meta.json");
}

/// Write a file of exactly `size_bytes` zero bytes into `run_dir` to control
/// the measured directory size.
///
/// Note: mirrors the inline helpers in retention.rs unit tests.
fn write_data_file(run_dir: &Path, filename: &str, size_bytes: usize) {
    let data = vec![0u8; size_bytes];
    std::fs::write(run_dir.join(filename), data).expect("write data file");
}

// ---------------------------------------------------------------------------
// IT-RP-01: 31-day-old completed run → deleted (age pruning)
// ---------------------------------------------------------------------------

/// A `Completed` run whose `started_at` is 31 days ago must be deleted when
/// the retention cap is 30 days.
#[tokio::test]
async fn it_rp_01_old_completed_run_deleted_by_age() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-old");

    write_meta(&run_dir, "run-old", RunStatus::Completed, 31);

    prune_runs(&[project_path], 30, 500).await;

    assert!(
        !run_dir.exists(),
        "Completed run 31 days old must be deleted when cap is 30 days"
    );
}

// ---------------------------------------------------------------------------
// IT-RP-02: 5-day-old completed run → survives (within cap)
// ---------------------------------------------------------------------------

/// A `Completed` run whose `started_at` is 5 days ago must survive when the
/// retention cap is 30 days.
#[tokio::test]
async fn it_rp_02_recent_completed_run_survives() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let run_dir = project_path.join(".claude").join("runs").join("run-recent");

    write_meta(&run_dir, "run-recent", RunStatus::Completed, 5);

    prune_runs(&[project_path], 30, 500).await;

    assert!(
        run_dir.exists(),
        "Completed run 5 days old must survive when cap is 30 days"
    );
}

// ---------------------------------------------------------------------------
// IT-RP-03: Pending/Running runs → never deleted even when old
// ---------------------------------------------------------------------------

/// `Pending` and `Running` runs must never be deleted by the pruner regardless
/// of how old they are or how tight the size cap is.
#[tokio::test]
async fn it_rp_03_active_runs_never_deleted() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let runs_dir = project_path.join(".claude").join("runs");

    let pending_dir = runs_dir.join("run-pending");
    let running_dir = runs_dir.join("run-running");

    // Both 100 days old — far beyond any reasonable cap.
    write_meta(&pending_dir, "run-pending", RunStatus::Pending, 100);
    write_meta(&running_dir, "run-running", RunStatus::Running, 100);

    // Tightest possible caps: 1-day age, 1 MB size.
    prune_runs(&[project_path], 1, 1).await;

    assert!(
        pending_dir.exists(),
        "Pending run must never be deleted by the retention pruner"
    );
    assert!(
        running_dir.exists(),
        "Running run must never be deleted by the retention pruner"
    );
}

// ---------------------------------------------------------------------------
// IT-RP-04: 600 MB of runs with 500 MB cap → oldest deleted
// ---------------------------------------------------------------------------

/// Three `Completed` runs of 200 MB each (600 MB total) with a 500 MB cap
/// must have the oldest deleted; the remaining two (400 MB) survive.
#[tokio::test]
async fn it_rp_04_size_cap_deletes_oldest_first() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let runs_dir = project_path.join(".claude").join("runs");

    let run_a = runs_dir.join("run-a"); // oldest: 3 days
    let run_b = runs_dir.join("run-b"); // middle: 2 days
    let run_c = runs_dir.join("run-c"); // newest: 1 day

    write_meta(&run_a, "run-a", RunStatus::Completed, 3);
    write_meta(&run_b, "run-b", RunStatus::Completed, 2);
    write_meta(&run_c, "run-c", RunStatus::Completed, 1);

    const MB_200: usize = 200 * 1024 * 1024;
    write_data_file(&run_a, "transcript.jsonl", MB_200);
    write_data_file(&run_b, "transcript.jsonl", MB_200);
    write_data_file(&run_c, "transcript.jsonl", MB_200);

    // 30-day age cap (none qualify for age pruning); 500 MB size cap.
    prune_runs(&[project_path], 30, 500).await;

    assert!(
        !run_a.exists(),
        "run-a (oldest, 200 MB) must be deleted to satisfy 500 MB cap"
    );
    assert!(
        run_b.exists(),
        "run-b must survive — 400 MB remaining is within 500 MB cap"
    );
    assert!(run_c.exists(), "run-c (newest) must survive");
}

// ---------------------------------------------------------------------------
// IT-RP-05: All three terminal statuses eligible for age pruning
// ---------------------------------------------------------------------------

/// `Completed`, `Failed`, and `Stopped` runs that are older than the retention
/// cap must all be deleted.
#[tokio::test]
async fn it_rp_05_all_terminal_statuses_pruned_by_age() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let runs_dir = project_path.join(".claude").join("runs");

    let completed_dir = runs_dir.join("run-completed");
    let failed_dir = runs_dir.join("run-failed");
    let stopped_dir = runs_dir.join("run-stopped");

    write_meta(&completed_dir, "run-completed", RunStatus::Completed, 31);
    write_meta(&failed_dir, "run-failed", RunStatus::Failed, 31);
    write_meta(&stopped_dir, "run-stopped", RunStatus::Stopped, 31);

    prune_runs(&[project_path], 30, 500).await;

    assert!(
        !completed_dir.exists(),
        "Completed run older than cap must be deleted"
    );
    assert!(
        !failed_dir.exists(),
        "Failed run older than cap must be deleted"
    );
    assert!(
        !stopped_dir.exists(),
        "Stopped run older than cap must be deleted"
    );
}

// ---------------------------------------------------------------------------
// IT-RP-06: Missing runs dir → no panic
// ---------------------------------------------------------------------------

/// When `.claude/runs/` does not exist the pruner must return without panic.
/// This is the normal state for freshly-registered projects.
#[tokio::test]
async fn it_rp_06_missing_runs_dir_no_panic() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    // Intentionally do NOT create .claude/runs/.

    // Must return without panic.
    prune_runs(&[project_path], 30, 500).await;
}

// ---------------------------------------------------------------------------
// IT-RP-07: Multiple projects, each pruned independently
// ---------------------------------------------------------------------------

/// Two separate project dirs each containing a 31-day-old run must both have
/// their old run deleted by a single pruner sweep.
#[tokio::test]
async fn it_rp_07_multiple_projects_pruned_independently() {
    let tmp_a = TempDir::new().expect("tempdir A");
    let tmp_b = TempDir::new().expect("tempdir B");

    let proj_a = tmp_a.path().to_path_buf();
    let proj_b = tmp_b.path().to_path_buf();

    let run_dir_a = proj_a.join(".claude").join("runs").join("run-a");
    let run_dir_b = proj_b.join(".claude").join("runs").join("run-b");

    write_meta(&run_dir_a, "run-a", RunStatus::Completed, 31);
    write_meta(&run_dir_b, "run-b", RunStatus::Failed, 31);

    prune_runs(&[proj_a, proj_b], 30, 500).await;

    assert!(!run_dir_a.exists(), "project A's old run must be deleted");
    assert!(!run_dir_b.exists(), "project B's old run must be deleted");
}

// ---------------------------------------------------------------------------
// IT-RP-08: Size pruning: oldest-first ordering preserved
// ---------------------------------------------------------------------------

/// Three `Completed` runs of 200 MB each with ages 3/2/1 days and a 500 MB
/// cap: the oldest (run-a, 3 days) must be deleted; run-b and run-c survive
/// at 400 MB total.  This is a stricter variant of IT-RP-04 that explicitly
/// verifies the ordering invariant by inspecting which specific run survives.
#[tokio::test]
async fn it_rp_08_size_pruning_preserves_oldest_first_order() {
    let tmp = TempDir::new().expect("tempdir");
    let project_path = tmp.path().to_path_buf();
    let runs_dir = project_path.join(".claude").join("runs");

    let run_a = runs_dir.join("run-a"); // oldest: 3 days ago
    let run_b = runs_dir.join("run-b"); // middle: 2 days ago
    let run_c = runs_dir.join("run-c"); // newest: 1 day ago

    write_meta(&run_a, "run-a", RunStatus::Completed, 3);
    write_meta(&run_b, "run-b", RunStatus::Completed, 2);
    write_meta(&run_c, "run-c", RunStatus::Completed, 1);

    const MB_200: usize = 200 * 1024 * 1024;
    write_data_file(&run_a, "raw.log", MB_200);
    write_data_file(&run_b, "raw.log", MB_200);
    write_data_file(&run_c, "raw.log", MB_200);

    // 30-day age cap; 500 MB size cap — triggers size pruning only.
    prune_runs(&[project_path], 30, 500).await;

    // Only the oldest (run-a) should be gone; remaining total = 400 MB.
    assert!(
        !run_a.exists(),
        "run-a (oldest, 3 days) must be deleted first to satisfy 500 MB cap"
    );
    assert!(
        run_b.exists(),
        "run-b (middle, 2 days) must survive — 400 MB ≤ 500 MB cap after run-a deletion"
    );
    assert!(run_c.exists(), "run-c (newest, 1 day) must survive");
}
