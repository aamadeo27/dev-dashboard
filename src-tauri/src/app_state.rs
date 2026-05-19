use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;

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
}
