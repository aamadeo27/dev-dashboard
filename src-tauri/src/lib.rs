pub(crate) mod app_state;
pub(crate) mod error;
pub mod ipc;
pub(crate) mod logging;
pub(crate) mod platform;
pub mod projects;
pub mod runs;
pub mod sequences;
pub mod settings;
pub mod usage;

use app_state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Resolve config directory.
    //
    // Priority:
    //   1. `DEV_DASHBOARD_CONFIG_DIR` env var (useful for dev/testing overrides).
    //   2. OS-standard config dir (`dirs::config_dir()`) joined with "dev-dashboard".
    //   3. Fallback: temp dir joined with "dev-dashboard" (should never be needed in
    //      practice, but avoids an unwrap panic on unusual OS configurations).
    let config_dir = if let Ok(override_dir) = std::env::var("DEV_DASHBOARD_CONFIG_DIR") {
        std::path::PathBuf::from(override_dir)
    } else {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("dev-dashboard")
    };

    let log_guard = logging::init_logging(&config_dir.join("logs"));

    tracing::info!(config_dir = %config_dir.display(), "config dir resolved");

    let state = AppState {
        config_dir,
        log_guard,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            ipc::commands::ping,
            ipc::commands::ping_error,
            ipc::commands::log_frontend_error,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri application failed to start — check logs for initialization errors");
}
