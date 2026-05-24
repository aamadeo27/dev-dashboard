use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Display mode for the project list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            parent_dir: None,
            claude_cli_path: None,
            git_poll_interval_secs: 10,
            usage_poll_interval_secs: 60,
            retention_days: 30,
            retention_size_mb: 500,
            view_mode: ViewMode::Grid,
        }
    }
}

/// Partial update payload for the `update_settings` command; only set fields are merged.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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

/// Persists and manages the application settings file.
///
/// The settings file is stored at `<config_dir>/settings.json`.
/// All mutations go through `patch()`, which validates ranges and then
/// atomically writes the file via a temp file + rename.
pub struct SettingsStore {
    settings: Settings,
}

impl SettingsStore {
    /// Returns a reference to the current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Path to the settings file inside the given config directory.
    fn settings_path(config_dir: &Path) -> PathBuf {
        config_dir.join("settings.json")
    }

    /// Path to the broken-file archive inside the given config directory.
    fn broken_path(config_dir: &Path) -> PathBuf {
        config_dir.join("settings.json.broken")
    }

    /// Path to the atomic temp file used during save.
    fn tmp_path(config_dir: &Path) -> PathBuf {
        config_dir.join("settings.json.tmp")
    }

    /// Load settings from `<config_dir>/settings.json`.
    ///
    /// - If the file does not exist, returns defaults (first-launch path).
    /// - If the file is corrupt/unparseable, logs a warning, archives the file
    ///   with a `.broken` suffix, and returns defaults.
    pub fn load(config_dir: &Path) -> Self {
        let path = Self::settings_path(config_dir);

        if !path.exists() {
            tracing::info!(path = %path.display(), "settings file not found; using defaults");
            return Self { settings: Settings::default() };
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to read settings file; using defaults"
                );
                return Self { settings: Settings::default() };
            }
        };

        match serde_json::from_str::<Settings>(&content) {
            Ok(settings) => {
                tracing::info!(path = %path.display(), "settings loaded");
                Self { settings }
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "settings file is corrupt; archiving and using defaults"
                );
                let broken = Self::broken_path(config_dir);
                if let Err(rename_err) = std::fs::rename(&path, &broken) {
                    tracing::warn!(
                        error = %rename_err,
                        "failed to archive corrupt settings file"
                    );
                }
                Self { settings: Settings::default() }
            }
        }
    }

    /// Atomically write `<config_dir>/settings.json`.
    ///
    /// Writes to a temp file in the same directory, then renames it over the
    /// target, so readers never see a partial write.
    pub async fn save(&self, config_dir: &Path) -> AppResult<()> {
        // Ensure the directory exists.
        tokio::fs::create_dir_all(config_dir).await?;

        let target = Self::settings_path(config_dir);
        let tmp_path = Self::tmp_path(config_dir);

        let json = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| AppError::Internal(format!("failed to serialize settings: {e}")))?;

        tokio::fs::write(&tmp_path, json).await?;
        tokio::fs::rename(&tmp_path, &target).await?;

        tracing::info!(path = %target.display(), "settings saved");
        Ok(())
    }

    /// Apply non-None fields from `patch`, validate ranges, then save.
    ///
    /// Returns `AppError::InvalidInput` if any value is out of range.
    pub async fn patch(&mut self, patch: SettingsPatch, config_dir: &Path) -> AppResult<()> {
        // Validate path fields before persisting.
        if let Some(ref p) = patch.claude_cli_path {
            if !p.is_absolute() {
                return Err(AppError::InvalidInput(
                    "claude_cli_path must be an absolute path".to_string(),
                ));
            }
            // On Windows, reject UNC and \\?\ paths to avoid network execution.
            #[cfg(target_os = "windows")]
            {
                let s = p.to_string_lossy();
                if s.starts_with("\\\\") {
                    return Err(AppError::InvalidInput(
                        "claude_cli_path must not be a UNC or network path".to_string(),
                    ));
                }
            }
            match tokio::fs::metadata(p).await {
                Ok(meta) if meta.is_file() => {}
                Ok(_) => return Err(AppError::InvalidInput(
                    format!("claude_cli_path is not a file: {}", p.display())
                )),
                Err(_) => return Err(AppError::InvalidInput(
                    format!("claude_cli_path does not exist: {}", p.display())
                )),
            }
        }
        if let Some(ref p) = patch.parent_dir {
            if !p.is_absolute() {
                return Err(AppError::InvalidInput(
                    "parent_dir must be an absolute path".to_string(),
                ));
            }
            if let Ok(meta) = tokio::fs::metadata(p).await {
                if !meta.is_dir() {
                    return Err(AppError::InvalidInput(
                        format!("parent_dir exists but is not a directory: {}", p.display())
                    ));
                }
            }
        }

        // Validate numeric ranges before mutating.
        if let Some(v) = patch.git_poll_interval_secs {
            if !(5..=3600).contains(&v) {
                return Err(AppError::InvalidInput(format!(
                    "git_poll_interval_secs must be between 5 and 3600, got {v}"
                )));
            }
        }
        if let Some(v) = patch.usage_poll_interval_secs {
            if !(30..=3600).contains(&v) {
                return Err(AppError::InvalidInput(format!(
                    "usage_poll_interval_secs must be between 30 and 3600, got {v}"
                )));
            }
        }
        if let Some(v) = patch.retention_days {
            if !(1..=90).contains(&v) {
                return Err(AppError::InvalidInput(format!(
                    "retention_days must be between 1 and 90, got {v}"
                )));
            }
        }
        if let Some(v) = patch.retention_size_mb {
            if !(50..=10_240).contains(&v) {
                return Err(AppError::InvalidInput(format!(
                    "retention_size_mb must be between 50 and 10240, got {v}"
                )));
            }
        }

        // Apply validated values.
        if let Some(v) = patch.parent_dir {
            self.settings.parent_dir = Some(v);
        }
        if let Some(v) = patch.claude_cli_path {
            self.settings.claude_cli_path = Some(v);
        }
        if let Some(v) = patch.git_poll_interval_secs {
            self.settings.git_poll_interval_secs = v;
        }
        if let Some(v) = patch.usage_poll_interval_secs {
            self.settings.usage_poll_interval_secs = v;
        }
        if let Some(v) = patch.retention_days {
            self.settings.retention_days = v;
        }
        if let Some(v) = patch.retention_size_mb {
            self.settings.retention_size_mb = v;
        }
        if let Some(v) = patch.view_mode {
            self.settings.view_mode = v;
        }

        self.save(config_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("temp dir")
    }

    #[tokio::test]
    async fn default_settings_when_file_missing() {
        let dir = temp_dir();
        let store = SettingsStore::load(dir.path());
        let s = store.settings();
        assert_eq!(s.git_poll_interval_secs, 10);
        assert_eq!(s.usage_poll_interval_secs, 60);
        assert_eq!(s.retention_days, 30);
        assert_eq!(s.retention_size_mb, 500);
        assert_eq!(s.view_mode, ViewMode::Grid);
        assert!(s.parent_dir.is_none());
        assert!(s.claude_cli_path.is_none());
    }

    #[tokio::test]
    async fn patch_applies_valid_values() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            git_poll_interval_secs: Some(120),
            usage_poll_interval_secs: Some(300),
            retention_days: Some(7),
            retention_size_mb: Some(100),
            view_mode: Some(ViewMode::List),
            ..SettingsPatch::default()
        };

        store.patch(patch, dir.path()).await.expect("patch should succeed");

        assert_eq!(store.settings().git_poll_interval_secs, 120);
        assert_eq!(store.settings().usage_poll_interval_secs, 300);
        assert_eq!(store.settings().retention_days, 7);
        assert_eq!(store.settings().retention_size_mb, 100);
        assert_eq!(store.settings().view_mode, ViewMode::List);
    }

    #[tokio::test]
    async fn patch_rejects_git_poll_below_min() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            git_poll_interval_secs: Some(2),
            ..SettingsPatch::default()
        };

        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("git_poll_interval_secs"));
    }

    #[tokio::test]
    async fn patch_rejects_git_poll_above_max() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            git_poll_interval_secs: Some(9999),
            ..SettingsPatch::default()
        };

        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn patch_rejects_usage_poll_below_min() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            usage_poll_interval_secs: Some(5),
            ..SettingsPatch::default()
        };

        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("usage_poll_interval_secs"));
    }

    #[tokio::test]
    async fn patch_rejects_retention_size_below_min() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            retention_size_mb: Some(10),
            ..SettingsPatch::default()
        };

        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("retention_size_mb"));
    }

    #[test]
    fn corrupt_json_falls_back_to_defaults_and_archives() {
        let dir = temp_dir();
        let settings_path = dir.path().join("settings.json");
        fs::write(&settings_path, b"{ not valid json !!!").expect("write corrupt file");

        let store = SettingsStore::load(dir.path());

        // Falls back to defaults.
        assert_eq!(store.settings().git_poll_interval_secs, 10);

        // Broken file archived.
        let broken_path = dir.path().join("settings.json.broken");
        assert!(broken_path.exists(), "broken file should have been archived");

        // Original file removed (renamed away).
        assert!(!settings_path.exists(), "original file should have been renamed");
    }

    #[tokio::test]
    async fn atomic_write_produces_valid_json() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            git_poll_interval_secs: Some(60),
            ..SettingsPatch::default()
        };
        store.patch(patch, dir.path()).await.expect("patch");

        let path = dir.path().join("settings.json");
        assert!(path.exists(), "settings file should exist after save");

        let content = fs::read_to_string(&path).expect("read back");
        let parsed: Settings = serde_json::from_str(&content).expect("must be valid JSON");
        assert_eq!(parsed.git_poll_interval_secs, 60);
    }

    #[tokio::test]
    async fn first_launch_creates_settings_file_on_save() {
        let dir = temp_dir();
        let store = SettingsStore::load(dir.path());

        // File does not exist yet.
        assert!(!dir.path().join("settings.json").exists());

        // Saving creates it.
        store.save(dir.path()).await.expect("save");
        assert!(dir.path().join("settings.json").exists());
    }

    #[tokio::test]
    async fn load_round_trips_saved_settings() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            retention_days: Some(14),
            view_mode: Some(ViewMode::List),
            ..SettingsPatch::default()
        };
        store.patch(patch, dir.path()).await.expect("patch");

        // Reload from disk.
        let store2 = SettingsStore::load(dir.path());
        assert_eq!(store2.settings().retention_days, 14);
        assert_eq!(store2.settings().view_mode, ViewMode::List);
    }

    #[tokio::test]
    async fn patch_accepts_retention_days_at_max() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            retention_days: Some(90),
            ..SettingsPatch::default()
        };
        store.patch(patch, dir.path()).await.expect("retention_days=90 must be accepted");
        assert_eq!(store.settings().retention_days, 90);
    }

    #[tokio::test]
    async fn patch_rejects_retention_days_above_max() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            retention_days: Some(91),
            ..SettingsPatch::default()
        };
        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("retention_days"));
    }

    #[tokio::test]
    async fn patch_accepts_retention_size_at_max() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            retention_size_mb: Some(10_240),
            ..SettingsPatch::default()
        };
        store.patch(patch, dir.path()).await.expect("retention_size_mb=10240 must be accepted");
        assert_eq!(store.settings().retention_size_mb, 10_240);
    }

    #[tokio::test]
    async fn patch_rejects_retention_size_above_max() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            retention_size_mb: Some(10_241),
            ..SettingsPatch::default()
        };
        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("retention_size_mb"));
    }

    #[tokio::test]
    async fn patch_rejects_relative_claude_cli_path() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            claude_cli_path: Some(std::path::PathBuf::from("relative/path/claude")),
            ..SettingsPatch::default()
        };
        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("claude_cli_path"));
    }

    #[tokio::test]
    async fn patch_rejects_relative_parent_dir() {
        let dir = temp_dir();
        let mut store = SettingsStore::load(dir.path());

        let patch = SettingsPatch {
            parent_dir: Some(std::path::PathBuf::from("relative/parent")),
            ..SettingsPatch::default()
        };
        let err = store.patch(patch, dir.path()).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert!(err.to_string().contains("parent_dir"));
    }
}
