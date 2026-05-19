/// Integration tests for DTO shape and serde correctness (T0.3).
///
/// These tests verify that each data-transfer object:
///   1. Can be constructed from a literal expression (compile-time type check).
///   2. Serializes to valid JSON without panicking.
///   3. Produces the specific JSON keys required by the frontend contract.
///
/// No filesystem I/O is performed. All `DateTime` values use
/// `chrono::DateTime::UNIX_EPOCH` (1970-01-01T00:00:00Z) for determinism.

use dev_dashboard_lib::ipc::commands::CliCheck;
use dev_dashboard_lib::projects::git::GitStatus;
use dev_dashboard_lib::projects::Project;
use dev_dashboard_lib::runs::{LaunchInput, Run, RunEvent, RunStatus, StepFailureChoice};
use dev_dashboard_lib::sequences::Sequence;
use dev_dashboard_lib::settings::{Settings, SettingsPatch, ViewMode};
use dev_dashboard_lib::usage::UsageSnapshot;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn epoch() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::UNIX_EPOCH
}

// ---------------------------------------------------------------------------
// TC-1: Project serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn project_serializes_to_json() {
    let p = Project {
        id: "01900000-0000-7000-8000-000000000001".to_string(),
        name: "my-project".to_string(),
        path: std::path::PathBuf::from("/home/user/projects/my-project"),
        tags: vec!["rust".to_string()],
        language: Some("Rust".to_string()),
        package_manager: None,
        added_at: epoch(),
        last_modified: None,
        is_missing: false,
    };
    let v = serde_json::to_value(&p).expect("Project must serialize");
    assert_eq!(v["name"], "my-project");
    assert_eq!(v["is_missing"], false);
    assert!(v["added_at"].is_string(), "added_at should be an ISO-8601 string");
    assert!(v["last_modified"].is_null());
}

// ---------------------------------------------------------------------------
// TC-2: Run serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn run_serializes_to_json() {
    let r = Run {
        id: "01900000-0000-7000-8000-000000000002".to_string(),
        project_id: "01900000-0000-7000-8000-000000000001".to_string(),
        project_path: std::path::PathBuf::from("/home/user/projects/my-project"),
        sequence_name: "build".to_string(),
        attached_md_path: None,
        started_at: epoch(),
        ended_at: None,
        status: RunStatus::Completed,
        exit_code: Some(0),
        pid: Some(12345),
        note: None,
    };
    let v = serde_json::to_value(&r).expect("Run must serialize");
    assert_eq!(v["sequence_name"], "build");
    assert_eq!(v["exit_code"], 0);
    assert!(v["pid"].is_number());
}

// ---------------------------------------------------------------------------
// TC-3: RunStatus::Completed serializes as "Completed" (no rename)
// ---------------------------------------------------------------------------

#[test]
fn run_status_completed_serializes_as_completed() {
    let status = RunStatus::Completed;
    let s = serde_json::to_string(&status).expect("RunStatus must serialize");
    assert_eq!(s, "\"Completed\"", "RunStatus::Completed must serialize as \"Completed\"");
}

#[test]
fn run_status_all_variants_use_pascal_case() {
    let cases = [
        (RunStatus::Pending, "\"Pending\""),
        (RunStatus::Running, "\"Running\""),
        (RunStatus::Completed, "\"Completed\""),
        (RunStatus::Failed, "\"Failed\""),
        (RunStatus::Stopped, "\"Stopped\""),
    ];
    for (variant, expected) in &cases {
        let s = serde_json::to_string(variant).expect("serialize");
        assert_eq!(&s, expected, "Unexpected serialization for RunStatus variant");
    }
}

// ---------------------------------------------------------------------------
// TC-4: RunEvent::AssistantText serializes with "type": "assistant_text"
// ---------------------------------------------------------------------------

#[test]
fn run_event_assistant_text_has_snake_case_type_discriminator() {
    let event = RunEvent::AssistantText {
        text: "Hello, world!".to_string(),
        ts: epoch(),
    };
    let v = serde_json::to_value(&event).expect("RunEvent must serialize");
    assert_eq!(
        v["type"], "assistant_text",
        "RunEvent::AssistantText must serialize with type = \"assistant_text\""
    );
    assert_eq!(v["text"], "Hello, world!");
}

// ---------------------------------------------------------------------------
// TC-5: RunEvent variants all carry their snake_case discriminator
// ---------------------------------------------------------------------------

#[test]
fn run_event_all_variants_have_correct_type_discriminator() {
    let ts = epoch();
    let null_val = serde_json::Value::Null;

    let cases: &[(&str, RunEvent)] = &[
        ("assistant_text", RunEvent::AssistantText { text: "t".into(), ts }),
        ("thinking", RunEvent::Thinking { text: "t".into(), ts }),
        (
            "tool_call",
            RunEvent::ToolCall {
                id: "id".into(),
                name: "n".into(),
                input: null_val.clone(),
                ts,
            },
        ),
        (
            "tool_result",
            RunEvent::ToolResult {
                call_id: "cid".into(),
                output: null_val.clone(),
                is_error: false,
                ts,
            },
        ),
        (
            "file_edit",
            RunEvent::FileEdit {
                path: "/f".into(),
                diff: "+a".into(),
                additions: 1,
                deletions: 0,
                ts,
            },
        ),
        ("user_input", RunEvent::UserInput { text: "q".into(), ts }),
        ("system", RunEvent::System { text: "s".into(), ts }),
        (
            "step_failed",
            RunEvent::StepFailed {
                step: "build".into(),
                message: "fail".into(),
                ts,
            },
        ),
        ("error", RunEvent::Error { message: "err".into(), ts }),
    ];

    for (expected_type, event) in cases {
        let v = serde_json::to_value(event).expect("RunEvent variant must serialize");
        assert_eq!(
            v["type"], *expected_type,
            "Wrong type discriminator for variant with expected type = {}",
            expected_type
        );
    }
}

// ---------------------------------------------------------------------------
// TC-6: Settings serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn settings_serializes_to_json() {
    let s = Settings {
        parent_dir: Some(std::path::PathBuf::from("/home/user/projects")),
        claude_cli_path: None,
        git_poll_interval_secs: 10,
        usage_poll_interval_secs: 60,
        retention_days: 30,
        retention_size_mb: 500,
        view_mode: ViewMode::Grid,
    };
    let v = serde_json::to_value(&s).expect("Settings must serialize");
    assert_eq!(v["git_poll_interval_secs"], 10);
    assert_eq!(v["view_mode"], "Grid");
    assert!(v["parent_dir"].is_string());
    assert!(v["claude_cli_path"].is_null());
}

// ---------------------------------------------------------------------------
// TC-7: SettingsPatch with all-None serializes cleanly
// ---------------------------------------------------------------------------

#[test]
fn settings_patch_all_none_serializes_to_json() {
    let patch = SettingsPatch {
        parent_dir: None,
        claude_cli_path: None,
        git_poll_interval_secs: None,
        usage_poll_interval_secs: None,
        retention_days: None,
        retention_size_mb: None,
        view_mode: None,
    };
    let v = serde_json::to_value(&patch).expect("SettingsPatch must serialize");
    assert!(v["parent_dir"].is_null());
    assert!(v["view_mode"].is_null());
}

// ---------------------------------------------------------------------------
// TC-8: UsageSnapshot serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn usage_snapshot_serializes_to_json() {
    let mut parsed = std::collections::BTreeMap::new();
    parsed.insert("tokens_used".to_string(), "1234".to_string());
    let snap = UsageSnapshot {
        fetched_at: epoch(),
        parsed,
        raw_stdout: "tokens_used: 1234\n".to_string(),
        available: true,
    };
    let v = serde_json::to_value(&snap).expect("UsageSnapshot must serialize");
    assert_eq!(v["available"], true);
    assert_eq!(v["parsed"]["tokens_used"], "1234");
}

// ---------------------------------------------------------------------------
// TC-9: GitStatus serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn git_status_serializes_to_json() {
    let gs = GitStatus {
        branch: Some("main".to_string()),
        is_clean: true,
        dirty_files: 0,
        ahead: 1,
        behind: 0,
        last_polled: epoch(),
        error: None,
    };
    let v = serde_json::to_value(&gs).expect("GitStatus must serialize");
    assert_eq!(v["branch"], "main");
    assert_eq!(v["is_clean"], true);
    assert_eq!(v["ahead"], 1);
}

// ---------------------------------------------------------------------------
// TC-10: LaunchInput serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn launch_input_serializes_to_json() {
    let li = LaunchInput {
        project_id: "01900000-0000-7000-8000-000000000001".to_string(),
        sequence_name: "deploy".to_string(),
        attached_md_path: None,
    };
    let v = serde_json::to_value(&li).expect("LaunchInput must serialize");
    assert_eq!(v["sequence_name"], "deploy");
    assert!(v["attached_md_path"].is_null());
}

// ---------------------------------------------------------------------------
// TC-11: StepFailureChoice variants serialize as PascalCase
// ---------------------------------------------------------------------------

#[test]
fn step_failure_choice_serializes_as_pascal_case() {
    let cases = [
        (StepFailureChoice::Retry, "\"Retry\""),
        (StepFailureChoice::Skip, "\"Skip\""),
        (StepFailureChoice::Abort, "\"Abort\""),
        (StepFailureChoice::Continue, "\"Continue\""),
    ];
    for (variant, expected) in &cases {
        let s = serde_json::to_string(variant).expect("serialize");
        assert_eq!(&s, expected, "Unexpected serialization for StepFailureChoice");
    }
}

// ---------------------------------------------------------------------------
// TC-12: CliCheck serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn cli_check_serializes_to_json() {
    let cc = CliCheck {
        found: true,
        resolved_path: Some(std::path::PathBuf::from("/usr/local/bin/claude")),
        version: Some("1.2.3".to_string()),
        error: None,
    };
    let v = serde_json::to_value(&cc).expect("CliCheck must serialize");
    assert_eq!(v["found"], true);
    assert_eq!(v["version"], "1.2.3");
    assert!(v["resolved_path"].is_string());
    assert!(v["error"].is_null());
}

// ---------------------------------------------------------------------------
// TC-13: Sequence serializes to valid JSON
// ---------------------------------------------------------------------------

#[test]
fn sequence_serializes_to_json() {
    let seq = Sequence {
        name: "build-and-test".to_string(),
        description: "Run build then test suite".to_string(),
        path: std::path::PathBuf::from("/home/user/.claude/sequences/build-and-test.md"),
        mtime: epoch(),
    };
    let v = serde_json::to_value(&seq).expect("Sequence must serialize");
    assert_eq!(v["name"], "build-and-test");
    assert!(v["path"].is_string());
    assert!(v["mtime"].is_string());
}
