use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(serde::Serialize, Clone)]
struct CliLostPayload {
    error: String,
}

/// Spawns the cli:lost background watcher task.
///
/// Polls the CLI every 60 seconds of focused window time.
/// Pauses while the window is unfocused (unfocus resets the 60s countdown).
/// Emits `cli:lost` Tauri event on found→not-found transition only — does NOT
/// repeat the event while the CLI remains absent.
/// Logs `tracing::warn!(component = "cli_detect", last_known_path, "cli lost mid-session")`
/// on each found→not-found transition (monitoring.md §2.13j).
pub fn start(
    app_handle: AppHandle,
    settings: Arc<tokio::sync::Mutex<crate::settings::SettingsStore>>,
) {
    // Assume focused at launch; on_window_event corrects this on the first
    // Focused(false) event if the app starts minimized or in the background.
    let is_focused = Arc::new(AtomicBool::new(true));
    let is_focused_for_event = is_focused.clone();

    // Track window focus state.
    // on_window_event fires for every window; we update the shared flag on any
    // Focused event regardless of which window caused it, which is correct for
    // a single-window app (this dashboard).
    if let Some(window) = app_handle.get_webview_window("main") {
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(focused) = event {
                is_focused_for_event.store(*focused, Ordering::Relaxed);
                tracing::debug!(
                    component = "window_focus",
                    state = if *focused { "focused" } else { "blurred" },
                    "window state change"
                );
            }
        });
    }

    tokio::spawn(async move {
        // Assume CLI was found at launch — the startup check (T1.3) would have
        // blocked the user from reaching the Dashboard if it was absent.
        // If the app somehow launched without CLI and reached Dashboard, the first
        // poll cycle will see not-found and emit cli:lost correctly.
        let mut last_found = true;
        let mut last_known_path: Option<PathBuf> = None;

        loop {
            // Wait 60 focused-window seconds before probing.
            // Check focus every 5 seconds; reset the countdown while unfocused
            // so we only count time the user is actively looking at the app.
            let mut focused_secs_elapsed: u64 = 0;
            while focused_secs_elapsed < 60 {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if is_focused.load(Ordering::Relaxed) {
                    focused_secs_elapsed += 5;
                } else {
                    // Window is unfocused — reset the countdown.
                    focused_secs_elapsed = 0;
                }
            }

            // Read the current CLI path from settings.
            let settings_path = {
                let store = settings.lock().await;
                store.settings().claude_cli_path.clone()
            };
            let resolved = crate::ipc::commands::resolve_cli_path(None, settings_path);

            // last_known_path is None on the very first not-found transition
            // (we never updated it because the CLI was found from launch).
            // The fallback to resolved.display() in the warn log is intentional.
            let found = probe_cli(&resolved).await;

            if found {
                if !last_found {
                    // Transition: not-found → found
                    tracing::info!(
                        component = "cli_detect",
                        restored_path = %resolved.display(),
                        "cli restored mid-session"
                    );
                }
                last_known_path = Some(resolved);
                last_found = true;
            } else if last_found {
                // Transition: found → not-found
                let path_str = last_known_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| resolved.display().to_string());

                tracing::warn!(
                    component = "cli_detect",
                    last_known_path = %path_str,
                    "cli lost mid-session"
                );

                // Emit Tauri event to the frontend. Errors are non-fatal — if
                // there is no listener the event is silently dropped.
                let _ = app_handle.emit(
                    crate::ipc::events::CLI_LOST,
                    CliLostPayload { error: path_str },
                );
                last_found = false;
            }
            // else: !found && !last_found — already emitted once; do not repeat.
        }
    });
}

/// Probe the CLI binary: spawn `<path> --version` and check that the process
/// is runnable within a 5-second timeout.
///
/// - Non-absolute paths are rejected immediately (bare "claude" PATH lookup
///   risks CWD hijack in a silent background task).
/// - `validate_cli_path` is called before spawning as a TOCTOU defense
///   (settings.json may have been tampered after patch() validated it).
/// - stdout/stderr are discarded (`Stdio::null()`) — this is a presence check,
///   not a version parse.
/// - Returns `true` if the child can be spawned and completes within 5 seconds;
///   `false` if the binary cannot be spawned (file not found, permission
///   denied, etc.).
/// - A 5-second timeout is enforced; if the binary hangs, the process is
///   killed (kill_on_drop) and the probe returns false (not-found).
async fn probe_cli(path: &std::path::Path) -> bool {
    // Reject bare/relative paths — PATH-lookup in a background task could be
    // hijacked via a malicious file in the working directory.
    if !path.is_absolute() {
        return false;
    }

    // TOCTOU defense: re-validate the path right before spawn in case
    // settings.json was tampered after the last patch() call.
    if crate::ipc::commands::validate_cli_path(path).await.is_err() {
        return false;
    }

    let result = tokio::process::Command::new(path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn();

    match result {
        Err(e) => {
            tracing::debug!(component = "cli_detect", path = %path.display(), error = %e, "probe spawn failed");
            false
        }
        Ok(mut child) => {
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(_)) => true,
                Ok(Err(e)) => {
                    tracing::debug!(component = "cli_detect", path = %path.display(), error = %e, "probe wait failed");
                    false
                }
                Err(_) => false, // timeout — kill_on_drop handles cleanup
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // probe_cli: tests that can run without a Tauri runtime
    // -----------------------------------------------------------------------

    /// A path that definitely does not exist must return false.
    #[tokio::test]
    async fn probe_cli_returns_false_for_nonexistent_path() {
        let path = if cfg!(target_os = "windows") {
            std::path::Path::new(r"C:\nonexistent_99999_watcher\claude.exe")
        } else {
            std::path::Path::new("/nonexistent_99999_watcher/claude")
        };
        let found = probe_cli(path).await;
        assert!(!found, "nonexistent binary must return false");
    }

    /// A non-absolute (relative/bare) path must return false immediately,
    /// without attempting a PATH lookup (CWD-hijack risk in a background task).
    #[tokio::test]
    async fn probe_cli_returns_false_for_non_absolute_path() {
        let found = probe_cli(std::path::Path::new("claude")).await;
        assert!(
            !found,
            "bare/relative path must return false without PATH lookup"
        );
    }

    /// An absolute path that does not exist must also return false.
    /// Mirrors the requirement stated in the task spec for /nonexistent paths.
    #[tokio::test]
    async fn probe_cli_returns_false_for_absolute_nonexistent_path() {
        let path = if cfg!(target_os = "windows") {
            std::path::Path::new(r"C:\this_path_cannot_exist_t1_6_test\bin\claude.exe")
        } else {
            std::path::Path::new("/this_path_cannot_exist_t1_6_test/bin/claude")
        };
        assert!(path.is_absolute(), "test path must be absolute");
        let found = probe_cli(path).await;
        assert!(!found, "absolute nonexistent path must return false");
    }

    /// A path pointing to a directory (not an executable) must return false.
    #[tokio::test]
    async fn probe_cli_returns_false_for_directory() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let found = probe_cli(tmp.path()).await;
        // Spawning a directory as a process always fails on all platforms.
        assert!(!found, "directory path must not be considered a valid CLI");
    }

    /// A path pointing to a non-executable regular file must return false.
    /// Covers the "file exists but is not runnable" case distinct from
    /// "file does not exist" (the spawn error is the same kind — EACCES or
    /// "not an executable" — but the OS path exercised is different).
    #[tokio::test]
    async fn probe_cli_returns_false_for_non_executable_file() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let fake_binary = tmp.path().join("not_a_binary.txt");
        std::fs::write(&fake_binary, b"not an elf").expect("write dummy file");

        // On Unix: validate_cli_path rejects (no execute bit means spawn fails) → false from spawn Err arm.
        // On Windows: validate_cli_path passes (file exists+is_file) → spawn fails (not Win32 app) → false from spawn Err arm.
        let found = probe_cli(&fake_binary).await;
        assert!(
            !found,
            "a non-executable regular file must not be considered a valid CLI"
        );
    }

    /// A real executable (the current test binary or a known system binary)
    /// must return true.  We use the current executable path from
    /// `std::env::current_exe()` which is guaranteed to exist and be runnable.
    #[tokio::test]
    async fn probe_cli_returns_true_for_real_executable() {
        let exe = std::env::current_exe().expect("current_exe must resolve");
        // The test binary will respond to --version with an error exit code or
        // unknown-flag output, but it will at least spawn successfully.
        // probe_cli only checks spawnability, not the exit code.
        let found = probe_cli(&exe).await;
        assert!(
            found,
            "current test executable must be considered a valid CLI"
        );
    }

    // -----------------------------------------------------------------------
    // Unit-level tests for logic that does not require a Tauri AppHandle
    // -----------------------------------------------------------------------

    /// The focused-seconds accumulator logic: verify that the counter resets
    /// when unfocused.  Simulated inline (not the actual loop) to avoid
    /// needing a Tauri runtime.
    #[test]
    fn focus_accumulator_resets_when_unfocused() {
        // Simulate the loop logic: 3 ticks focused, 1 unfocused, 3 more focused.
        let mut elapsed: u64 = 0;
        let ticks: Vec<bool> = vec![true, true, true, false, true, true, true];
        for focused in ticks {
            if focused {
                elapsed += 5;
            } else {
                elapsed = 0;
            }
        }
        // After unfocus-reset the counter restarts: 3 ticks * 5 = 15
        assert_eq!(elapsed, 15, "counter must restart from zero after unfocus");
    }

    /// All unfocused ticks: counter must never leave 0, meaning the probe
    /// would never fire.  Guards against an off-by-one where an unfocused
    /// tick still increments before resetting.
    #[test]
    fn focus_accumulator_stays_zero_when_always_unfocused() {
        let mut elapsed: u64 = 0;
        let ticks: Vec<bool> = vec![false, false, false, false, false];
        for focused in ticks {
            if focused {
                elapsed += 5;
            } else {
                elapsed = 0;
            }
        }
        assert_eq!(
            elapsed, 0,
            "counter must remain 0 when all ticks are unfocused"
        );
    }

    /// Exactly 12 focused ticks × 5 s = 60 s must satisfy the `< 60` loop
    /// condition and exit.  This confirms the boundary value is correct
    /// (60 is the threshold in the while condition).
    #[test]
    fn focus_accumulator_reaches_threshold_after_exactly_60_seconds() {
        let mut elapsed: u64 = 0;
        let threshold: u64 = 60;
        for _ in 0..12 {
            elapsed += 5; // 12 × 5 = 60
        }
        // The while condition is `elapsed < 60`; at 60 the loop exits.
        assert!(
            elapsed >= threshold,
            "12 focused ticks must reach the 60s threshold"
        );
        assert_eq!(elapsed, threshold, "exactly 60s after 12 ticks");
    }

    // -----------------------------------------------------------------------
    // State machine logic: simulated inline (no Tauri runtime required)
    //
    // The state machine lives inside the `start()` spawn closure and cannot
    // be called directly without a Tauri AppHandle.  The logic is extracted
    // verbatim into each test and exercised as pure Rust to verify correctness
    // of the transition rules documented in the task spec table.
    // -----------------------------------------------------------------------

    /// found→not-found transition: when last_found is true and the probe
    /// returns false, last_found must flip to false.
    /// The emit/log side-effects cannot be asserted here (require AppHandle),
    /// but the state mutation is the invariant the rest of the system depends on.
    #[test]
    fn state_machine_found_to_not_found_sets_last_found_false() {
        let mut last_found = true;
        let mut last_known_path: Option<PathBuf> = Some(PathBuf::from("/usr/bin/claude"));
        let resolved = PathBuf::from("/usr/bin/claude");
        let found = false; // probe result

        // Reproduce the state machine logic from start():
        if found {
            last_known_path = Some(resolved.clone());
            last_found = true;
        } else if last_found {
            // Transition: found → not-found.
            // (log + emit would happen here in the real code)
            last_found = false;
        }
        // else: already not-found — no change.

        assert!(
            !last_found,
            "last_found must be false after found→not-found transition"
        );
        // last_known_path must NOT be updated on a not-found result.
        assert_eq!(
            last_known_path,
            Some(PathBuf::from("/usr/bin/claude")),
            "last_known_path must not change on a not-found probe"
        );
    }

    /// not-found→not-found: when last_found is already false and the probe
    /// returns false again, last_found stays false and no new transition
    /// fires (the `else if last_found` guard prevents duplicate events).
    #[test]
    fn state_machine_not_found_to_not_found_no_state_change() {
        let mut last_found = false;
        let mut last_known_path: Option<PathBuf> = None;
        let resolved = PathBuf::from("/usr/bin/claude");
        let found = false; // probe still not-found

        // Reproduce state machine:
        if found {
            last_known_path = Some(resolved.clone());
            last_found = true;
        } else if last_found {
            last_found = false;
        }
        // else: already not-found — no change.

        assert!(
            !last_found,
            "last_found must remain false (no duplicate transition)"
        );
        assert!(
            last_known_path.is_none(),
            "last_known_path must remain None when probe stays not-found"
        );
    }

    /// not-found→found restoration: when last_found is false and the probe
    /// returns true, last_found must flip back to true and last_known_path
    /// must be updated to the resolved path.
    #[test]
    fn state_machine_not_found_to_found_restores_last_found() {
        let mut last_found = false;
        let mut last_known_path: Option<PathBuf> = None;
        let resolved = PathBuf::from("/usr/bin/claude");
        let found = true; // probe succeeded — CLI is back

        // Reproduce state machine:
        if found {
            // Restoration branch (last_found was false → log info in real code).
            last_known_path = Some(resolved.clone());
            last_found = true;
        } else if last_found {
            last_found = false;
        }

        assert!(
            last_found,
            "last_found must be true after not-found→found restoration"
        );
        assert_eq!(
            last_known_path,
            Some(PathBuf::from("/usr/bin/claude")),
            "last_known_path must be updated to the resolved path on restoration"
        );
    }

    /// found→found (steady state): when last_found is true and the probe
    /// returns true, last_found stays true and last_known_path is updated.
    /// No event or log should fire — the `else if last_found` branch is not
    /// entered because `found` is true.
    #[test]
    fn state_machine_found_to_found_updates_last_known_path() {
        let mut last_found = true;
        let mut last_known_path: Option<PathBuf> = Some(PathBuf::from("/old/claude"));
        let resolved = PathBuf::from("/new/resolved/claude");
        let found = true;

        if found {
            last_known_path = Some(resolved.clone());
            last_found = true;
        } else if last_found {
            last_found = false;
        }

        assert!(
            last_found,
            "last_found must remain true in steady-found state"
        );
        assert_eq!(
            last_known_path,
            Some(PathBuf::from("/new/resolved/claude")),
            "last_known_path must be updated to the latest resolved path on each found tick"
        );
    }

    // -----------------------------------------------------------------------
    // resolve_cli_path delegation
    // -----------------------------------------------------------------------

    /// Verify that `resolve_cli_path` falls through to the settings path when
    /// no override is given — confirms the watcher uses the correct resolution
    /// logic (delegating to the shared helper).
    #[test]
    fn watcher_path_resolution_uses_settings_path() {
        let settings_path = std::path::PathBuf::from("/usr/local/bin/claude");
        let resolved = crate::ipc::commands::resolve_cli_path(None, Some(settings_path.clone()));
        assert_eq!(resolved, settings_path);
    }

    /// Verify that `resolve_cli_path` falls back to bare `"claude"` when
    /// settings has no path set.
    #[test]
    fn watcher_path_resolution_falls_back_to_claude() {
        let resolved = crate::ipc::commands::resolve_cli_path(None, None);
        assert_eq!(resolved, std::path::PathBuf::from("claude"));
    }

    // -----------------------------------------------------------------------
    // last_known_path / warn log path selection
    // -----------------------------------------------------------------------

    /// Verify the last_known_path fallback logic: when last_known_path is None
    /// the warning uses the resolved path string.
    #[test]
    fn path_str_uses_resolved_when_last_known_path_is_none() {
        let last_known_path: Option<PathBuf> = None;
        let resolved = std::path::PathBuf::from("/usr/bin/claude");
        let path_str = last_known_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| resolved.display().to_string());
        assert_eq!(path_str, "/usr/bin/claude");
    }

    /// Verify the last_known_path fallback logic: when last_known_path is Some
    /// it takes priority over the resolved path.
    #[test]
    fn path_str_prefers_last_known_path_when_set() {
        let last_known_path: Option<PathBuf> =
            Some(std::path::PathBuf::from("/cached/path/claude"));
        let resolved = std::path::PathBuf::from("/new/resolved/claude");
        let path_str = last_known_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| resolved.display().to_string());
        assert_eq!(path_str, "/cached/path/claude");
    }

    /// Edge: last_known_path is None and resolved is also the bare "claude"
    /// fallback (no settings path set).  path_str must equal "claude".
    #[test]
    fn path_str_fallback_when_both_none_and_bare_claude() {
        let last_known_path: Option<PathBuf> = None;
        let resolved = std::path::PathBuf::from("claude"); // bare fallback
        let path_str = last_known_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| resolved.display().to_string());
        assert_eq!(
            path_str, "claude",
            "bare 'claude' fallback must be used in warn log when no path is known"
        );
    }
}
