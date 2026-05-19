// UsageProbe goes here

/// Snapshot of Claude CLI token/cost usage; `available` is false when the CLI invocation failed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct UsageSnapshot {
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub parsed: std::collections::BTreeMap<String, String>,
    pub raw_stdout: String,
    pub available: bool,
}
