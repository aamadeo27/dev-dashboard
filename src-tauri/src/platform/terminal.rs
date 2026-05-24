use std::path::PathBuf;

use crate::error::{AppError, AppResult};

/// Open the project directory in the OS default terminal application.
///
/// Uses `tauri_plugin_opener::open_path` to trigger the OS default handler
/// for the directory path (which is the system terminal on most platforms).
///
/// On failure: emits a `toast:show` error event via `app` and returns
/// `AppError::Io`.
pub async fn open_in_terminal_impl(
    id: &str,
    path: PathBuf,
    app: &tauri::AppHandle,
) -> AppResult<()> {
    tracing::info!(component = "platform", id = ?id, path = ?path, "open_in_terminal invoked");

    let result = tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    });

    if let Err(ref e) = result {
        tracing::warn!(component = "platform", id = ?id, error = ?e, "open_in_terminal failed");
        app.emit(crate::ipc::events::TOAST_SHOW, serde_json::json!({
            "kind": "error",
            "title": "Cannot open terminal",
            "body": e.to_string(),
        }))
        .ok();
    }

    result
}
