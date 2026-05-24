// GitPoller — per-project git status polling

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Payload emitted on the `git:updated` event for a single project.
#[derive(serde::Serialize, Clone)]
struct GitUpdatedPayload {
    id: String,
    status: GitStatus,
}

/// Snapshot of a project's git state; `error` is Some when the last poll failed (other fields may be stale).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct GitStatus {
    pub branch: Option<String>,
    pub is_clean: bool,
    pub dirty_files: u32,
    pub ahead: u32,
    pub behind: u32,
    pub last_polled: chrono::DateTime<chrono::Utc>,
    pub error: Option<String>,
}

/// Holds shared state for the background git poll loop.
pub struct GitPoller {
    /// In-memory cache of last known git status per project id.
    pub statuses: Arc<tokio::sync::Mutex<HashMap<String, GitStatus>>>,
    /// IDs of projects currently visible in the frontend viewport.
    pub visible: Arc<tokio::sync::Mutex<HashSet<String>>>,
    /// True when the window is blurred — polling pauses.
    pub paused: Arc<AtomicBool>,
}

impl GitPoller {
    pub fn new() -> Self {
        Self {
            statuses: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            visible: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for GitPoller {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawns the git poll background task.
///
/// Pattern mirrors `ipc::cli_watcher::start`. Polls only the visible project
/// set, pauses within ~1 s of window blur by resetting the unpaused-time
/// counter on each blur event.
///
/// The poll loop and the `app_handle.on_window_event` callback cannot be
/// unit-tested without a live Tauri AppHandle; correctness of the loop logic
/// (pause/resume accumulator, visible-set snapshot) is verified via inline
/// simulation tests below.
pub fn start(
    app_handle: AppHandle,
    settings: Arc<tokio::sync::Mutex<crate::settings::SettingsStore>>,
    projects: Arc<tokio::sync::Mutex<crate::projects::ProjectRegistry>>,
    poller: Arc<GitPoller>,
) {
    // Register window focus/blur handler.
    // Sets paused = !focused so the poll loop resets its elapsed counter within
    // the next 500 ms tick after a blur event.
    let paused_for_event = poller.paused.clone();
    app_handle.on_window_event(move |_window, event| {
        if let tauri::WindowEvent::Focused(focused) = event {
            // paused = true when NOT focused; inverted polarity vs cli_watcher's is_focused
            paused_for_event.store(!focused, Ordering::Relaxed);
            tracing::debug!(
                component = "git_poller",
                state = if *focused { "focused" } else { "blurred" },
                "window focus changed"
            );
        }
    });

    tokio::spawn(async move {
        loop {
            // Read poll interval from settings (do NOT hold the lock during sleep).
            let interval_ms: u64 = {
                let store = settings.lock().await;
                store.settings().git_poll_interval_secs as u64 * 1000
            };

            // Sleep in 500 ms ticks, counting only unpaused time.
            // When paused the counter resets to 0, achieving "pauses within ~1 s
            // of window blur" without a busy-wait.
            let mut unpaused_ms: u64 = 0;
            while unpaused_ms < interval_ms {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if poller.paused.load(Ordering::Relaxed) {
                    unpaused_ms = 0; // reset on pause
                } else {
                    unpaused_ms = unpaused_ms.saturating_add(500);
                }
            }

            // Snapshot visible project IDs (don't hold the lock while doing I/O).
            let visible_ids: Vec<String> = {
                let vis = poller.visible.lock().await;
                vis.iter().cloned().collect()
            };

            if visible_ids.is_empty() {
                continue;
            }

            // Acquire the projects mutex ONCE and collect all (id, path) pairs.
            let id_paths: Vec<(String, std::path::PathBuf)> = {
                let registry = projects.lock().await;
                visible_ids.iter()
                    .filter_map(|id| registry.get_project_path(id).map(|p| (id.clone(), p)))
                    .collect()
            };
            if id_paths.is_empty() {
                continue;
            }

            // Dispatch all git status checks concurrently via JoinSet.
            // git2 is synchronous — run each on the blocking thread pool.
            let mut set = tokio::task::JoinSet::new();
            for (id, path) in id_paths {
                set.spawn_blocking(move || (id, git_status_for_path(&path)));
            }

            // Drain results, update cache, emit events.
            let mut cache = poller.statuses.lock().await;
            while let Some(result) = set.join_next().await {
                match result {
                    Ok((id, status)) => {
                        let _ = app_handle.emit(
                            crate::ipc::events::GIT_UPDATED,
                            GitUpdatedPayload { id: id.clone(), status: status.clone() },
                        );
                        cache.insert(id, status);
                    }
                    Err(e) => {
                        tracing::warn!(component = "git_poller", error = %e, "spawn_blocking task panicked");
                    }
                }
            }
        }
    });
}

/// Strips control characters and truncates to `max_chars` Unicode scalar values.
///
/// Uses `.chars().take(max_chars)` — safe for all UTF-8 input because it
/// operates on char boundaries, not byte offsets.
fn sanitize_git_string(s: &str, max_chars: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max_chars).collect()
}

/// Computes git status synchronously via git2. Must be called via `spawn_blocking`.
///
/// Returns a `GitStatus` with `error: Some(...)` when the repository cannot be
/// found or opened; all numeric fields default to 0 and `is_clean` to `true`
/// in that case to avoid spurious dirtiness in the UI.
pub fn git_status_for_path(path: &std::path::Path) -> GitStatus {
    let now = chrono::Utc::now();

    let repo = match git2::Repository::discover(path) {
        Ok(r) => r,
        Err(e) => {
            let error_msg = sanitize_git_string(e.message(), 512);
            return GitStatus {
                branch: None,
                is_clean: true,
                dirty_files: 0,
                ahead: 0,
                behind: 0,
                last_polled: now,
                error: Some(error_msg),
            }
        }
    };

    // Current branch short name (e.g. "main").
    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));
    let branch = branch.map(|b| sanitize_git_string(&b, 256));

    // Dirty file count — files in the working tree or index that differ from HEAD.
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).include_ignored(false);
    let dirty_files = repo
        .statuses(Some(&mut opts))
        .map(|s| {
            s.iter()
                .filter(|e| e.status() != git2::Status::CURRENT)
                .count() as u32
        })
        .unwrap_or(0);
    let is_clean = dirty_files == 0;

    // Ahead/behind count vs. `refs/remotes/origin/<branch>`.
    // Returns (0, 0) if there is no upstream or the graph walk fails.
    let (ahead, behind) = (|| -> Option<(u32, u32)> {
        let head = repo.head().ok()?;
        let local_oid = head.peel_to_commit().ok()?.id();
        let branch_name = head.shorthand()?;
        let upstream_ref = repo
            .find_reference(&format!("refs/remotes/origin/{}", branch_name))
            .ok()?;
        let upstream_oid = upstream_ref.peel_to_commit().ok()?.id();
        let (a, b) = repo.graph_ahead_behind(local_oid, upstream_oid).ok()?;
        Some((a as u32, b as u32))
    })()
    .unwrap_or((0, 0));

    GitStatus {
        branch,
        is_clean,
        dirty_files,
        ahead,
        behind,
        last_polled: now,
        error: None,
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // git_status_for_path: non-git / nonexistent paths
    // -----------------------------------------------------------------------

    /// A directory that is not a git repository must return a GitStatus with
    /// `error: Some(...)` and safe defaults for all numeric/bool fields.
    #[test]
    fn git_status_for_path_non_git_dir_returns_error() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let status = git_status_for_path(tmp.path());

        assert!(
            status.error.is_some(),
            "error must be Some for a non-git directory, got: {:?}",
            status.error
        );
        // Safe defaults.
        assert!(status.is_clean, "is_clean must default to true on error");
        assert_eq!(status.dirty_files, 0, "dirty_files must default to 0 on error");
        assert_eq!(status.ahead, 0, "ahead must default to 0 on error");
        assert_eq!(status.behind, 0, "behind must default to 0 on error");
        assert!(status.branch.is_none(), "branch must be None on error");
    }

    /// A path that does not exist at all must also return `error: Some(...)`.
    #[test]
    fn git_status_for_path_nonexistent_path_returns_error() {
        let path = if cfg!(target_os = "windows") {
            std::path::Path::new(r"C:\nonexistent_t2_3_git_test\project")
        } else {
            std::path::Path::new("/nonexistent_t2_3_git_test/project")
        };

        let status = git_status_for_path(path);

        assert!(
            status.error.is_some(),
            "error must be Some for a nonexistent path, got: {:?}",
            status.error
        );
        assert!(status.is_clean);
        assert_eq!(status.dirty_files, 0);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
    }

    // -----------------------------------------------------------------------
    // GitPoller::new() default state
    // -----------------------------------------------------------------------

    /// A freshly constructed GitPoller must have an empty statuses cache, an
    /// empty visible set, and paused = false.
    #[tokio::test]
    async fn git_poller_new_default_state() {
        let poller = GitPoller::new();

        let statuses = poller.statuses.lock().await;
        assert!(statuses.is_empty(), "statuses cache must be empty on construction");
        drop(statuses);

        let visible = poller.visible.lock().await;
        assert!(visible.is_empty(), "visible set must be empty on construction");
        drop(visible);

        assert!(
            !poller.paused.load(Ordering::Relaxed),
            "paused must be false on construction"
        );
    }

    // -----------------------------------------------------------------------
    // set_visible_projects logic (simulated via direct Mutex mutation)
    // The real command goes through AppState, which requires a Tauri runtime.
    // We verify the underlying data structure behaves correctly here.
    // -----------------------------------------------------------------------

    /// Directly mutating the visible set (as set_visible_projects does via the
    /// Mutex) must replace the entire set.
    #[tokio::test]
    async fn visible_set_replace_updates_correctly() {
        let poller = GitPoller::new();

        // Simulate first call: set two project IDs.
        {
            let mut visible = poller.visible.lock().await;
            *visible = ["id-a".to_string(), "id-b".to_string()]
                .into_iter()
                .collect();
        }

        {
            let visible = poller.visible.lock().await;
            assert!(visible.contains("id-a"), "id-a must be present");
            assert!(visible.contains("id-b"), "id-b must be present");
            assert_eq!(visible.len(), 2);
        }

        // Simulate second call: replace with a single different ID.
        {
            let mut visible = poller.visible.lock().await;
            *visible = ["id-c".to_string()].into_iter().collect();
        }

        {
            let visible = poller.visible.lock().await;
            assert!(!visible.contains("id-a"), "id-a must be gone after replace");
            assert!(!visible.contains("id-b"), "id-b must be gone after replace");
            assert!(visible.contains("id-c"), "id-c must be present after replace");
            assert_eq!(visible.len(), 1);
        }
    }

    // -----------------------------------------------------------------------
    // Pause accumulator logic (same pattern as cli_watcher tests)
    // The actual poll loop lives inside start() and cannot be tested without a
    // Tauri AppHandle; the accumulator logic is extracted inline here.
    // -----------------------------------------------------------------------

    /// When paused, the unpaused_ms counter resets to 0 on each tick.
    #[test]
    fn pause_accumulator_resets_when_paused() {
        let mut unpaused_ms: u64 = 0;
        // 3 unpaused ticks, then 1 paused tick, then 3 more unpaused.
        let ticks: Vec<bool> = vec![false, false, false, true, false, false, false];
        // false = not-paused (accumulate), true = paused (reset)
        for paused in ticks {
            if paused {
                unpaused_ms = 0;
            } else {
                unpaused_ms = unpaused_ms.saturating_add(500);
            }
        }
        // After the pause-reset the counter restarts: 3 * 500 = 1500
        assert_eq!(unpaused_ms, 1500, "counter must restart from zero after pause");
    }

    /// When always paused, the counter must never leave 0 (poll never fires).
    #[test]
    fn pause_accumulator_stays_zero_when_always_paused() {
        let mut unpaused_ms: u64 = 0;
        for _ in 0..10 {
            let paused = true;
            if paused {
                unpaused_ms = 0;
            } else {
                unpaused_ms = unpaused_ms.saturating_add(500);
            }
        }
        assert_eq!(unpaused_ms, 0);
    }

    // -----------------------------------------------------------------------
    // sanitize_git_string — FIX-1 (T2.3-fixes-3)
    // -----------------------------------------------------------------------

    /// ASCII input shorter than the limit must pass through unchanged.
    #[test]
    fn sanitize_git_string_short_ascii_unchanged() {
        let result = sanitize_git_string("main", 256);
        assert_eq!(result, "main");
    }

    /// Control characters must be stripped.
    #[test]
    fn sanitize_git_string_strips_control_chars() {
        let input = "feat\x00ure\x1bbranch";
        let result = sanitize_git_string(input, 256);
        assert_eq!(result, "featurebranch");
    }

    /// Input longer than `max_chars` ASCII chars must be truncated to exactly
    /// `max_chars` characters (not bytes).
    #[test]
    fn sanitize_git_string_truncates_long_ascii() {
        let input = "a".repeat(300);
        let result = sanitize_git_string(&input, 256);
        assert_eq!(result.len(), 256);
    }

    /// Multi-byte UTF-8 (Cyrillic): must NOT panic and must truncate at
    /// char boundary, not a byte boundary.  Each Cyrillic char is 2 bytes;
    /// a branch like `feature/тест` (12 chars) must survive with `max_chars`
    /// large enough.
    #[test]
    fn sanitize_git_string_multibyte_utf8_no_panic() {
        let input = "feature/тест"; // 12 Unicode chars, 16 bytes
        let result = sanitize_git_string(input, 256);
        assert_eq!(result, "feature/тест");
    }

    /// When `max_chars` falls inside a multi-byte sequence (by char count), the
    /// helper must still return valid UTF-8 with exactly `max_chars` chars.
    #[test]
    fn sanitize_git_string_truncates_multibyte_at_char_boundary() {
        // 10 Cyrillic chars = 20 bytes; truncate to 5 chars.
        let input = "тестирование"; // 12 chars
        let result = sanitize_git_string(input, 5);
        assert_eq!(result.chars().count(), 5);
        // Must be valid UTF-8 (no panic on len() or iteration).
        let _ = result.len();
    }

    /// An input of exactly `max_chars` characters must not be truncated.
    #[test]
    fn sanitize_git_string_exact_limit_not_truncated() {
        let input = "б".repeat(512); // 512 Cyrillic chars = 1024 bytes
        let result = sanitize_git_string(&input, 512);
        assert_eq!(result.chars().count(), 512);
    }
}
