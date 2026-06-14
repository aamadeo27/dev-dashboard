// RetentionPruner — age/size cap enforcement.
//
// Entry points:
//   `run(project_paths, retention_days, retention_size_mb)` — one-shot sweep.
//   `start(app_handle)` — spawns the 24-hour background timer loop.
//
// For each project the sweep:
//   1. Collects all terminal-status run dirs (Completed / Failed / Stopped).
//   2. Sorts them by `started_at` ascending (oldest first).
//   3. Age pruning: deletes runs older than `retention_days` from today.
//   4. Size pruning: while the total size of remaining runs exceeds
//      `retention_size_mb * 1024 * 1024`, deletes the oldest remaining run.
//
// All I/O errors are logged at warn level and do not propagate.

use std::path::{Path, PathBuf};

use chrono::Utc;
use tauri::Manager;

use crate::app_state::AppState;

use super::{Run, RunStatus};

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Run a single retention sweep across all registered project paths.
///
/// # Arguments
/// * `project_paths` — canonicalized paths of all registered projects.
/// * `retention_days` — runs older than this many days from now are deleted.
/// * `retention_size_mb` — per-project size cap in mebibytes.
pub async fn run(project_paths: &[PathBuf], retention_days: u32, retention_size_mb: u32) {
    tracing::info!(
        component = "retention_pruner",
        project_count = project_paths.len(),
        retention_days,
        retention_size_mb,
        "retention pruner started"
    );

    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let size_cap_bytes: u64 = retention_size_mb as u64 * 1024 * 1024;

    let mut total_pruned: u32 = 0;
    let projects_total = project_paths.len();

    for project_path in project_paths {
        let pruned = prune_project(project_path, cutoff, size_cap_bytes).await;
        total_pruned += pruned;
    }

    tracing::info!(
        component = "retention_pruner",
        total_pruned,
        projects_total,
        "retention pruner finished"
    );
}

/// Spawn the 24-hour background timer loop that repeatedly calls `run`.
pub async fn start(app_handle: tauri::AppHandle) {
    // Detached background timer; the JoinHandle is intentionally dropped.
    tokio::spawn(async move {
        loop {
            // sleep first: startup sweep already ran; first periodic sweep fires 24 h later.
            tokio::time::sleep(tokio::time::Duration::from_secs(86_400)).await;

            let (project_paths, retention_days, retention_size_mb) = {
                let state = app_handle.state::<AppState>();

                let project_paths: Vec<PathBuf> = {
                    let projects = state.projects.lock().await;
                    let list = projects.list_projects().await;
                    list.into_iter().map(|p| p.path).collect()
                };

                let (retention_days, retention_size_mb) = {
                    let settings = state.settings.lock().await;
                    let s = settings.settings();
                    (s.retention_days, s.retention_size_mb)
                };

                (project_paths, retention_days, retention_size_mb)
            };

            run(&project_paths, retention_days, retention_size_mb).await;
        }
    });
}

// ---------------------------------------------------------------------------
// Per-project pruning
// ---------------------------------------------------------------------------

/// Prune a single project's runs dir. Returns the count of deleted run dirs.
async fn prune_project(
    project_path: &Path,
    cutoff: chrono::DateTime<Utc>,
    size_cap_bytes: u64,
) -> u32 {
    let runs_dir = project_path.join(".claude").join("runs");

    // Collect terminal-state run candidates.
    let mut candidates = match collect_terminal_runs(&runs_dir).await {
        Some(v) => v,
        // warn already emitted in collect_terminal_runs; skip project.
        None => return 0,
    };

    // Sort oldest first.
    candidates.sort_by_key(|c| c.started_at);

    let mut pruned: u32 = 0;

    // ── Age pruning ──────────────────────────────────────────────────────────
    // Partition into deleted-by-age (removed from disk) and remaining.
    let mut remaining: Vec<RunCandidate> = Vec::new();
    for candidate in candidates {
        if candidate.started_at < cutoff {
            if delete_run_dir(&candidate.run_dir, &candidate.run_id, "age", project_path).await {
                pruned += 1;
            }
        } else {
            remaining.push(candidate);
        }
    }

    // ── Size pruning ─────────────────────────────────────────────────────────
    // Compute total size of remaining runs; delete oldest first until under cap.
    let mut total_size: u64 = compute_total_size(&remaining);

    // `remaining` is already sorted oldest-first; iterate by index so we can pop.
    let mut idx = 0;
    while total_size > size_cap_bytes && idx < remaining.len() {
        let candidate = &remaining[idx];
        let dir_size = candidate.dir_size;
        if delete_run_dir(&candidate.run_dir, &candidate.run_id, "size", project_path).await {
            pruned += 1;
            total_size = total_size.saturating_sub(dir_size);
        }
        idx += 1;
    }

    pruned
}

// ---------------------------------------------------------------------------
// Candidate collection
// ---------------------------------------------------------------------------

/// A terminal run that is eligible for deletion.
struct RunCandidate {
    run_dir: PathBuf,
    run_id: String,
    started_at: chrono::DateTime<Utc>,
    /// Measured size in bytes (0 if unreadable — conservative: don't delete).
    dir_size: u64,
}

/// Walk `runs_dir` and return all terminal-status run candidates, or `None` if
/// the directory cannot be read (logged at warn; not an error for callers).
async fn collect_terminal_runs(runs_dir: &Path) -> Option<Vec<RunCandidate>> {
    let mut entries = match tokio::fs::read_dir(runs_dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(e) => {
            tracing::warn!(
                component = "retention_pruner",
                path = %runs_dir.display(),
                error = %e,
                "failed to read runs directory; skipping project"
            );
            return None;
        }
    };

    let mut candidates = Vec::new();

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(
                    component = "retention_pruner",
                    error = %e,
                    "error iterating runs directory entry; skipping"
                );
                continue;
            }
        };

        let run_dir = entry.path();

        match entry.file_type().await {
            Ok(ft) if ft.is_dir() => {}
            _ => continue,
        }

        let meta_path = run_dir.join("meta.json");
        let run = match read_meta(&meta_path).await {
            Some(r) => r,
            None => continue,
        };

        // Only terminal statuses are eligible for pruning.
        if !matches!(
            run.status,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Stopped
        ) {
            continue;
        }

        let dir_size = measure_dir_size(&run_dir).await;

        candidates.push(RunCandidate {
            run_id: run.id.clone(),
            started_at: run.started_at,
            run_dir,
            dir_size,
        });
    }

    Some(candidates)
}

// ---------------------------------------------------------------------------
// Filesystem helpers
// ---------------------------------------------------------------------------

/// Read and deserialize `meta.json`, returning `None` on any error (logged at warn).
async fn read_meta(meta_path: &Path) -> Option<Run> {
    let bytes = match tokio::fs::read(meta_path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(
                component = "retention_pruner",
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
                component = "retention_pruner",
                path = %meta_path.display(),
                error = %e,
                "failed to parse meta.json; skipping"
            );
            None
        }
    }
}

/// Sum the file sizes of all direct children of `run_dir`.
/// Unreadable entries contribute 0 (conservative: don't delete on partial failure).
///
/// Run directories are guaranteed to be flat by the contracts (TranscriptWriter
/// creates only `meta.json`, `transcript.jsonl`, and `raw.log` — all top-level
/// files; see `docs/kb/contracts/run-event.md`). No recursive walk is needed.
async fn measure_dir_size(run_dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(run_dir).await {
        Ok(e) => e,
        Err(_) => return 0,
    };

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => continue,
        };

        if let Ok(meta) = tokio::fs::metadata(entry.path()).await {
            if meta.is_file() {
                total += meta.len();
            }
        }
    }

    total
}

/// Sum `dir_size` for every candidate in the slice.
fn compute_total_size(candidates: &[RunCandidate]) -> u64 {
    candidates.iter().map(|c| c.dir_size).sum()
}

/// Delete `run_dir` and log the result. Returns `true` if deletion succeeded.
async fn delete_run_dir(run_dir: &Path, run_id: &str, reason: &str, project_path: &Path) -> bool {
    match tokio::fs::remove_dir_all(run_dir).await {
        Ok(()) => {
            tracing::info!(
                component = "retention_pruner",
                run_id = %run_id,
                reason = %reason,
                project = %project_path.display(),
                "deleted run dir"
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                component = "retention_pruner",
                run_id = %run_id,
                reason = %reason,
                project = %project_path.display(),
                error = %e,
                "failed to delete run dir; skipping"
            );
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Write `meta.json` with given status and `started_at` offset in days from now.
    /// Negative `days_offset` = in the past, positive = in the future.
    fn write_meta(run_dir: &Path, id: &str, status: RunStatus, days_ago: i64) {
        std::fs::create_dir_all(run_dir).expect("create run_dir");
        let started_at = Utc::now() - chrono::Duration::days(days_ago);
        let run = Run {
            id: id.to_string(),
            project_id: "proj-test".to_string(),
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
        let json = serde_json::to_string_pretty(&run).expect("serialize");
        std::fs::write(run_dir.join("meta.json"), json).expect("write meta.json");
    }

    /// Write `size_bytes` of data into a file inside `run_dir` to control measured size.
    fn write_data_file(run_dir: &Path, filename: &str, size_bytes: usize) {
        let data = vec![0u8; size_bytes];
        std::fs::write(run_dir.join(filename), data).expect("write data file");
    }

    // ── Test 1: old terminal runs are deleted ────────────────────────────────

    #[tokio::test]
    async fn old_terminal_runs_are_pruned_by_age() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // 31-day-old completed run — older than 30-day cap.
        let old_dir = runs_dir.join("run-old");
        write_meta(&old_dir, "run-old", RunStatus::Completed, 31);

        run(&[project_path], 30, 500).await;

        assert!(
            !old_dir.exists(),
            "run older than retention_days must be deleted"
        );
    }

    // ── Test 2: recent terminal runs are NOT deleted ─────────────────────────

    #[tokio::test]
    async fn recent_terminal_runs_are_not_pruned() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // 5-day-old run with a 30-day cap — must survive.
        let recent_dir = runs_dir.join("run-recent");
        write_meta(&recent_dir, "run-recent", RunStatus::Completed, 5);

        run(&[project_path], 30, 500).await;

        assert!(
            recent_dir.exists(),
            "run within retention_days must not be deleted"
        );
    }

    // ── Test 3: non-terminal runs are never deleted ──────────────────────────

    #[tokio::test]
    async fn active_runs_are_never_deleted() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // Both are very old (100 days) but non-terminal — must never be deleted.
        let pending_dir = runs_dir.join("run-pending");
        let running_dir = runs_dir.join("run-running");
        write_meta(&pending_dir, "run-pending", RunStatus::Pending, 100);
        write_meta(&running_dir, "run-running", RunStatus::Running, 100);

        // Tiny size cap (1 MB) to ensure size pruning would also fire if eligible.
        run(&[project_path], 1, 1).await;

        assert!(
            pending_dir.exists(),
            "Pending run must never be deleted by retention pruner"
        );
        assert!(
            running_dir.exists(),
            "Running run must never be deleted by retention pruner"
        );
    }

    // ── Test 4: size cap — oldest pruned to bring under cap ──────────────────

    #[tokio::test]
    async fn size_cap_prunes_oldest_first() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // Three runs of 200 MB each = 600 MB total; cap is 500 MB.
        // All are 1 day old — within a 30-day retention window.
        // After pruning, oldest should be removed to bring total ≤ 500 MB.
        let run_a = runs_dir.join("run-a");
        let run_b = runs_dir.join("run-b");
        let run_c = runs_dir.join("run-c");

        // run-a is oldest (3 days), run-b is middle (2 days), run-c is newest (1 day).
        write_meta(&run_a, "run-a", RunStatus::Completed, 3);
        write_meta(&run_b, "run-b", RunStatus::Completed, 2);
        write_meta(&run_c, "run-c", RunStatus::Completed, 1);

        // 200 MB each.
        const MB_200: usize = 200 * 1024 * 1024;
        write_data_file(&run_a, "transcript.jsonl", MB_200);
        write_data_file(&run_b, "transcript.jsonl", MB_200);
        write_data_file(&run_c, "transcript.jsonl", MB_200);

        // 30-day age cap (none are old enough), 500 MB size cap.
        run(&[project_path], 30, 500).await;

        // Oldest (run-a) must be deleted; run-b and run-c survive (400 MB ≤ 500 MB cap).
        assert!(
            !run_a.exists(),
            "run-a (oldest, 200 MB) must be deleted to satisfy size cap"
        );
        assert!(
            run_b.exists(),
            "run-b must survive after run-a deletion brings total to 400 MB"
        );
        assert!(run_c.exists(), "run-c (newest) must survive");
    }

    // ── Test 5: combined age + size pruning ──────────────────────────────────

    #[tokio::test]
    async fn age_pruning_fires_before_size_pruning() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // run-old: 35 days old, 200 MB — should be deleted by age rule first.
        // run-mid: 10 days old, 200 MB — within age cap.
        // run-new: 1 day old, 200 MB — within age cap.
        // Total after age pruning: 400 MB; cap = 500 MB → no size pruning needed.
        let run_old = runs_dir.join("run-old");
        let run_mid = runs_dir.join("run-mid");
        let run_new = runs_dir.join("run-new");

        write_meta(&run_old, "run-old", RunStatus::Failed, 35);
        write_meta(&run_mid, "run-mid", RunStatus::Completed, 10);
        write_meta(&run_new, "run-new", RunStatus::Stopped, 1);

        const MB_200: usize = 200 * 1024 * 1024;
        write_data_file(&run_old, "raw.log", MB_200);
        write_data_file(&run_mid, "raw.log", MB_200);
        write_data_file(&run_new, "raw.log", MB_200);

        run(&[project_path], 30, 500).await;

        assert!(!run_old.exists(), "run-old must be deleted by age rule");
        assert!(
            run_mid.exists(),
            "run-mid (10 days, 200 MB) must survive — 400 MB ≤ 500 MB cap"
        );
        assert!(run_new.exists(), "run-new must survive");
    }

    // ── Test 6: missing runs dir → no panic ─────────────────────────────────

    #[tokio::test]
    async fn missing_runs_dir_does_not_panic() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        // Intentionally do NOT create .claude/runs/.

        // Must not panic.
        run(&[project_path], 30, 500).await;
    }

    // ── Test 7: oldest-first ordering — all over-age runs deleted ────────────

    #[tokio::test]
    async fn all_over_age_runs_are_deleted() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        // Three runs, all 31-40 days old — all should be deleted by the 30-day rule.
        let run_x = runs_dir.join("run-x");
        let run_y = runs_dir.join("run-y");
        let run_z = runs_dir.join("run-z");

        write_meta(&run_x, "run-x", RunStatus::Completed, 31);
        write_meta(&run_y, "run-y", RunStatus::Failed, 35);
        write_meta(&run_z, "run-z", RunStatus::Stopped, 40);

        run(&[project_path], 30, 500).await;

        assert!(!run_x.exists(), "run-x (31 days) must be deleted");
        assert!(!run_y.exists(), "run-y (35 days) must be deleted");
        assert!(!run_z.exists(), "run-z (40 days) must be deleted");
    }

    // ── Test 8: empty project list → no panic ────────────────────────────────

    #[tokio::test]
    async fn empty_project_list_does_not_panic() {
        run(&[], 30, 500).await;
    }

    // ── Test 9: mixed terminal statuses are all eligible for pruning ──────────

    #[tokio::test]
    async fn all_terminal_statuses_are_eligible_for_age_pruning() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let runs_dir = project_path.join(".claude").join("runs");

        let completed_dir = runs_dir.join("run-completed");
        let failed_dir = runs_dir.join("run-failed");
        let stopped_dir = runs_dir.join("run-stopped");

        write_meta(&completed_dir, "run-completed", RunStatus::Completed, 31);
        write_meta(&failed_dir, "run-failed", RunStatus::Failed, 31);
        write_meta(&stopped_dir, "run-stopped", RunStatus::Stopped, 31);

        run(&[project_path], 30, 500).await;

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

    // ── Test 10: size pruning skips non-existent dirs gracefully ─────────────

    #[tokio::test]
    async fn corrupt_meta_json_is_skipped() {
        let tmp = TempDir::new().expect("tempdir");
        let project_path = tmp.path().to_path_buf();
        let run_dir = project_path
            .join(".claude")
            .join("runs")
            .join("run-corrupt");
        std::fs::create_dir_all(&run_dir).expect("create run_dir");
        std::fs::write(run_dir.join("meta.json"), b"{ not valid json !!! }")
            .expect("write corrupt");

        // Must not panic; corrupt entry is skipped.
        run(&[project_path], 30, 500).await;
    }
}
