// ProjectRegistry goes here

pub(crate) mod scanner;
pub mod git;

/// A registered project entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct Project {
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub path: std::path::PathBuf,
    pub tags: Vec<String>,
    pub language: Option<String>,
    pub package_manager: Option<String>,
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub is_missing: bool,
}
