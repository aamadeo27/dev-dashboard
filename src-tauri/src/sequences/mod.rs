// SequenceLoader goes here

/// A Claude sequence definition discovered on disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct Sequence {
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub path: std::path::PathBuf,
    pub mtime: chrono::DateTime<chrono::Utc>,
}
