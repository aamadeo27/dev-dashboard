// RunManager goes here

pub mod manager;
pub(crate) mod orphan;
pub(crate) mod parser;
pub(crate) mod retention;
pub(crate) mod session;
pub(crate) mod transcript;
pub use manager::RunManager;

/// Public entry-point for the retention pruner sweep, exposed for integration
/// testing.  Production callers should use `retention::run` directly through
/// the `lib.rs` setup hook.
pub async fn prune_runs(
    project_paths: &[std::path::PathBuf],
    retention_days: u32,
    retention_size_mb: u32,
) {
    retention::run(project_paths, retention_days, retention_size_mb).await
}

/// Public entry-point for the orphan-reaper sweep, exposed for integration
/// testing.  Production callers should use `orphan::run` directly through the
/// `lib.rs` setup hook.
pub async fn reap_orphans(
    project_paths: &[std::path::PathBuf],
    claude_cli_path: Option<&std::path::Path>,
) {
    orphan::run(project_paths, claude_cli_path).await
}

/// Lifecycle state of a run; serializes as the variant name (`"Pending"`, `"Running"`, …).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Stopped,
}

/// A persisted run record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct Run {
    pub id: String,
    pub project_id: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub project_path: std::path::PathBuf,
    pub sequence_name: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub attached_md_path: Option<std::path::PathBuf>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
    pub note: Option<String>,
    /// Set by the system when a run is aborted via `respond_to_step_failure`.
    /// Distinct from `note` (user-facing) — this is system-set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_note: Option<String>,
    /// For re-invoked runs: the `run_id` of the original run that triggered
    /// the Retry / Skip / Continue-fallback that spawned this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_of: Option<String>,
}

/// A single structured event emitted during a run; tagged union on `type` (snake_case).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub enum RunEvent {
    AssistantText {
        text: String,
        ts: chrono::DateTime<chrono::Utc>,
    },
    Thinking {
        text: String,
        ts: chrono::DateTime<chrono::Utc>,
    },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
        ts: chrono::DateTime<chrono::Utc>,
    },
    ToolResult {
        call_id: String,
        output: serde_json::Value,
        is_error: bool,
        ts: chrono::DateTime<chrono::Utc>,
    },
    FileEdit {
        path: String,
        diff: String,
        additions: u32,
        deletions: u32,
        ts: chrono::DateTime<chrono::Utc>,
    },
    UserInput {
        text: String,
        ts: chrono::DateTime<chrono::Utc>,
    },
    System {
        text: String,
        ts: chrono::DateTime<chrono::Utc>,
    },
    StepFailed {
        step: String,
        message: String,
        ts: chrono::DateTime<chrono::Utc>,
    },
    Error {
        message: String,
        ts: chrono::DateTime<chrono::Utc>,
    },
}

/// Input payload for the `launch_run` command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct LaunchInput {
    pub project_id: String,
    pub sequence_name: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub attached_md_path: Option<std::path::PathBuf>,
}

/// User's response when a sequence step fails.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub enum StepFailureChoice {
    Retry,
    Skip,
    Abort,
    Continue,
}
