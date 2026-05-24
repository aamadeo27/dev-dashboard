use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_appender::non_blocking::WorkerGuard;

use crate::projects::ProjectRegistry;
use crate::runs::RunManager;
use crate::sequences::SequenceLoader;
use crate::settings::SettingsStore;

/// Application state held by Tauri for the process lifetime.
///
/// Stored via `tauri::Manager::manage(state)` in `lib.rs::run()` and accessed
/// in command handlers via `tauri::State<'_, AppState>`.
pub struct AppState {
    /// Resolved OS config directory for this application.
    ///
    /// Resolution order (see `lib.rs`):
    /// 1. `DEV_DASHBOARD_CONFIG_DIR` env var if set.
    /// 2. `dirs::config_dir()` / `"dev-dashboard"` subdirectory.
    /// 3. Fallback: `std::env::temp_dir()` / `"dev-dashboard"`.
    pub config_dir: PathBuf,

    /// Keeps the non-blocking log writer alive for the application lifetime.
    ///
    /// Dropping this guard flushes and terminates the background log writer.
    /// Must be held for the entire process lifetime.
    pub log_guard: WorkerGuard,

    /// Persisted application settings, protected by an async mutex so that
    /// concurrent Tauri commands can safely read and mutate settings.
    pub settings: Arc<Mutex<SettingsStore>>,

    /// In-memory project registry, backed by `projects.json` in the config
    /// directory. Protected by an async mutex for concurrent command access.
    pub projects: Arc<Mutex<ProjectRegistry>>,

    /// Shared state for the background git poll loop (T2.3).
    ///
    /// Holds the per-project status cache, the visible-project set, and the
    /// pause flag. Wrapped in Arc so it can be cloned into the poll task.
    pub git_poller: Arc<crate::projects::git::GitPoller>,

    /// In-memory loader for `.claude/sequences/*.md` files (T3.1).
    ///
    /// Caches sequence lists per project_id, invalidated on directory mtime
    /// change. Protected by an async mutex for concurrent command access.
    pub sequence_loader: Arc<Mutex<SequenceLoader>>,

    /// Manager for active Claude CLI child process sessions (T4.3).
    ///
    /// Tracks all in-flight runs.  Commands `launch_run`, `stop_run`, and
    /// `send_input` access this to create, cancel, and write to sessions.
    pub run_manager: Arc<Mutex<RunManager>>,
}
