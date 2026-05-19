// SettingsStore goes here

/// Display mode for the project list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub enum ViewMode {
    Grid,
    List,
}

/// Application-wide settings persisted to disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct Settings {
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub parent_dir: Option<std::path::PathBuf>,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub claude_cli_path: Option<std::path::PathBuf>,
    pub git_poll_interval_secs: u32,
    pub usage_poll_interval_secs: u32,
    pub retention_days: u32,
    pub retention_size_mb: u32,
    pub view_mode: ViewMode,
}

/// Partial update payload for the `update_settings` command; only set fields are merged.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct SettingsPatch {
    #[cfg_attr(feature = "export-bindings", ts(type = "string | null"))]
    pub parent_dir: Option<std::path::PathBuf>,
    #[cfg_attr(feature = "export-bindings", ts(type = "string | null"))]
    pub claude_cli_path: Option<std::path::PathBuf>,
    pub git_poll_interval_secs: Option<u32>,
    pub usage_poll_interval_secs: Option<u32>,
    pub retention_days: Option<u32>,
    pub retention_size_mb: Option<u32>,
    pub view_mode: Option<ViewMode>,
}
