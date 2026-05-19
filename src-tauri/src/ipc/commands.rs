use crate::error::AppResult;

/// CLI probe result returned by the `verify_claude_cli` command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct CliCheck {
    pub found: bool,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub resolved_path: Option<std::path::PathBuf>,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// Health-check command; returns `"pong"` to confirm IPC is operational.
#[tauri::command]
pub async fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

// TODO(release): remove — exists only to smoke-test AppError IPC serialization.
#[tauri::command]
pub async fn ping_error() -> AppResult<String> {
    Err(crate::error::AppError::NotFound("ping_error_test".to_string()))
}

/// Forwards a frontend error into the structured log (monitoring.md §1.3k).
#[tauri::command]
pub async fn log_frontend_error(
    message: String,
    stack: Option<String>,
    route: Option<String>,
) {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    tracing::error!(
        component = "frontend",
        source = "frontend",
        kind = "frontend",
        correlation_id = %correlation_id,
        stack = stack.as_deref().unwrap_or(""),
        route = route.as_deref().unwrap_or(""),
        "{}", message
    );
}
