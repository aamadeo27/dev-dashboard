use crate::error::AppResult;

/// Smoke-test command. Returns `"pong"` to confirm IPC is operational.
///
/// Uses `Result<String, String>` rather than `AppResult` intentionally —
/// this command exists only as a health-check and has no domain error cases.
#[tauri::command]
pub async fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

// Smoke-test command: demonstrates AppError IPC serialization. Remove in production if desired.
#[tauri::command]
pub async fn ping_error() -> AppResult<String> {
    Err(crate::error::AppError::NotFound("ping_error_test".to_string()))
}
