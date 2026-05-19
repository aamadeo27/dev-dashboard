// GitPoller — per-project git status polling

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
