pub(crate) mod app_state;
pub(crate) mod error;
pub mod ipc;
pub(crate) mod logging;
pub(crate) mod platform;
pub(crate) mod projects;
pub(crate) mod runs;
pub(crate) mod sequences;
pub(crate) mod settings;
pub(crate) mod usage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // TODO: resolve config_dir before builder (requires tauri::api::path or dirs crate)
    // For now, init logging with a temporary path; wire proper path in T9.x when AppState lands.
    let log_dir = std::env::temp_dir().join("dev-dashboard").join("logs");
    let _log_guard = logging::init_logging(&log_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("Tauri application failed to start — check logs for initialization errors");
}
