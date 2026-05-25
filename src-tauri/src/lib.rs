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

use std::sync::Arc;

use app_state::AppState;
use settings::SettingsStore;

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

    // Load settings before constructing AppState so any first-launch defaults
    // are applied (and optionally persisted) before the first IPC command fires.
    let settings_store = SettingsStore::load(&config_dir);

    // Write defaults to disk on first launch so settings.json always exists after startup.
    if !config_dir.join("settings.json").exists() {
        let json = serde_json::to_string_pretty(settings_store.settings()).unwrap_or_default();
        if let Err(e) = std::fs::write(config_dir.join("settings.json"), json) {
            tracing::warn!(error = %e, "failed to write initial settings file on first launch");
        }
    }

    let projects_registry = projects::ProjectRegistry::load(&config_dir);

    let git_poller = Arc::new(crate::projects::git::GitPoller::new());

    let state = AppState {
        config_dir,
        log_guard,
        settings: Arc::new(tokio::sync::Mutex::new(settings_store)),
        projects: Arc::new(tokio::sync::Mutex::new(projects_registry)),
        git_poller: git_poller.clone(),
        sequence_loader: sequences::SequenceLoader::new_arc(),
        run_manager: runs::RunManager::new_arc(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .setup(|app| {
            // ── OrphanReaper (T4.5) — must complete before setup returns ──
            // Collect registered project paths and the configured CLI path, then
            // run the async sweep to completion (blocking on the JoinHandle) so
            // all orphaned runs are marked failed before any IPC handler fires.
            {
                let settings = app.state::<AppState>().settings.clone();
                let projects = app.state::<AppState>().projects.clone();
                let handle = tauri::async_runtime::spawn(async move {
                    // Clone data before any await point to avoid holding the Mutex
                    // guard across async boundaries.
                    let project_paths: Vec<std::path::PathBuf> = {
                        let projects_guard = projects.lock().await;
                        // Collect the paths without awaiting inside the lock.
                        let list = projects_guard.list_projects().await;
                        list.into_iter().map(|p| p.path).collect()
                    };
                    let cli_path: Option<std::path::PathBuf> = {
                        let settings_guard = settings.lock().await;
                        settings_guard.settings().claude_cli_path.clone()
                    };
                    runs::orphan::run(&project_paths, cli_path.as_deref()).await;
                });
                // Block on the sweep to ensure it completes before setup returns.
                tauri::async_runtime::block_on(handle)
                    .map_err(|e| format!("orphan reaper join error: {}", e))?;
            }

            // ── RetentionPruner startup sweep (T4.6) — fire-and-forget ──────
            // Best-effort: pruning is not startup-critical so we do not block.
            {
                let settings = app.state::<AppState>().settings.clone();
                let projects = app.state::<AppState>().projects.clone();
                tauri::async_runtime::spawn(async move {
                    let project_paths: Vec<std::path::PathBuf> = {
                        let projects_guard = projects.lock().await;
                        let list = projects_guard.list_projects().await;
                        list.into_iter().map(|p| p.path).collect()
                    };
                    let (retention_days, retention_size_mb) = {
                        let settings_guard = settings.lock().await;
                        let s = settings_guard.settings();
                        (s.retention_days, s.retention_size_mb)
                    };
                    runs::retention::run(&project_paths, retention_days, retention_size_mb).await;
                });
            }

            // ── RetentionPruner 24h timer (T4.6) ─────────────────────────────
            tauri::async_runtime::spawn(runs::retention::start(app.handle().clone()));

            // Start the background CLI-loss watcher (T1.6).
            // manage() has already run so app.state() is available here.
            let settings = app.state::<AppState>().settings.clone();
            ipc::cli_watcher::start(app.handle().clone(), settings);

            // Start the git poller (T2.3).
            let settings = app.state::<AppState>().settings.clone();
            let projects = app.state::<AppState>().projects.clone();
            let git_poller_arc = app.state::<AppState>().git_poller.clone();
            crate::projects::git::start(app.handle().clone(), settings, projects, git_poller_arc);

            Ok(())
        })
        .invoke_handler({
            #[cfg(debug_assertions)]
            { tauri::generate_handler![
                ipc::commands::ping,
                ipc::commands::ping_error,
                ipc::commands::log_frontend_error,
                ipc::commands::get_settings,
                ipc::commands::update_settings,
                ipc::commands::open_logs_folder,
                ipc::commands::verify_claude_cli,
                ipc::commands::list_projects,
                ipc::commands::add_project,
                ipc::commands::remove_project,
                ipc::commands::relocate_project,
                ipc::commands::set_project_tags,
                ipc::commands::rename_project,
                ipc::commands::scan_project,
                ipc::commands::open_in_editor,
                ipc::commands::open_in_terminal,
                ipc::commands::get_git_status,
                ipc::commands::set_visible_projects,
                ipc::commands::list_sequences,
                ipc::commands::refresh_sequences,
                ipc::commands::launch_run,
                ipc::commands::stop_run,
                ipc::commands::send_input,
            ] }
            #[cfg(not(debug_assertions))]
            { tauri::generate_handler![
                ipc::commands::ping,
                ipc::commands::log_frontend_error,
                ipc::commands::get_settings,
                ipc::commands::update_settings,
                ipc::commands::open_logs_folder,
                ipc::commands::verify_claude_cli,
                ipc::commands::list_projects,
                ipc::commands::add_project,
                ipc::commands::remove_project,
                ipc::commands::relocate_project,
                ipc::commands::set_project_tags,
                ipc::commands::scan_project,
                ipc::commands::open_in_editor,
                ipc::commands::open_in_terminal,
                ipc::commands::get_git_status,
                ipc::commands::set_visible_projects,
                ipc::commands::list_sequences,
                ipc::commands::refresh_sequences,
                ipc::commands::launch_run,
                ipc::commands::stop_run,
                ipc::commands::send_input,
            ] }
        })
        .run(tauri::generate_context!())
        .expect("Tauri application failed to start — check logs for initialization errors");
}
