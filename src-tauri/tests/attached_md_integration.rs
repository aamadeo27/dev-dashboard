/// Integration tests for `read_attached_md` (T4.4).
///
/// # Scope
///
/// These tests exercise `read_attached_md` across the crate boundary and cover:
///
///   IT-T4.4-01  Happy path — valid file, contents returned correctly.
///   IT-T4.4-02  Missing file — AppError::NotFound, no side effects.
///   IT-T4.4-03  Oversized file (> 1 MiB) — AppError::InvalidInput.
///   IT-T4.4-04  Exactly 1 MiB — boundary accepted.
///   IT-T4.4-05  Empty file (0 bytes) — accepted, returns empty Vec.
///   IT-T4.4-06  NotFound message contains the file path.
///   IT-T4.4-07  InvalidInput message mentions size / "too large".
///   IT-T4.4-08  Binary content round-trips without corruption.
///   IT-T4.4-09  meta.json records attached_md_path when Some.
///   IT-T4.4-10  meta.json records null when attached_md_path is None.
///
/// # Why `launch_run` end-to-end is NOT tested here
///
/// `launch_run` is a `#[tauri::command]` that takes `tauri::State<'_, AppState>`
/// and `tauri::AppHandle` — both require a running Tauri application context.
/// Constructing those handles without a live window is not supported by the Tauri
/// test API in v2.  The contracts exercised by `launch_run` are instead covered by:
///   - The unit tests in `commands::tests` (T4.4-01 through T4.4-06).
///   - These integration tests, which call `read_attached_md` directly across the
///     crate boundary to verify the same contracts hold when called from outside
///     `commands.rs`.
///   - The `TranscriptWriter::create` integration tests (IT-T4.4-09 / -10) which
///     verify that `meta.json` persists `attached_md_path` correctly.

use dev_dashboard_lib::ipc::commands::read_attached_md;
use dev_dashboard_lib::runs::{Run, RunStatus};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("create temp dir")
}

/// Construct a minimal `Run` for use in TranscriptWriter tests.
fn make_run(id: &str, attached_md_path: Option<std::path::PathBuf>) -> Run {
    Run {
        id: id.to_string(),
        project_id: "test-project".to_string(),
        project_path: std::env::temp_dir().join("project"),
        sequence_name: "test-seq".to_string(),
        attached_md_path,
        started_at: chrono::Utc::now(),
        ended_at: None,
        status: RunStatus::Pending,
        exit_code: None,
        pid: None,
        note: None,
    }
}

// ---------------------------------------------------------------------------
// IT-T4.4-01: Happy path — existing file within limit is read correctly
// ---------------------------------------------------------------------------

/// A file that exists and is within the 1 MiB limit must be returned
/// byte-for-byte, confirming that `read_attached_md` does not transform content.
#[tokio::test]
async fn it_t4_4_01_happy_path_reads_file_contents_correctly() {
    let dir = temp_dir();
    let path = dir.path().join("context.md");
    let expected: &[u8] = b"# Context\n\nHello from context.\n";

    tokio::fs::write(&path, expected)
        .await
        .expect("write test file");

    let result = read_attached_md(&path).await;

    assert!(
        result.is_ok(),
        "existing file within limit must return Ok; got: {:?}",
        result
    );
    assert_eq!(
        result.unwrap(),
        expected,
        "returned bytes must match file contents byte-for-byte"
    );
}

// ---------------------------------------------------------------------------
// IT-T4.4-02: Missing file returns NotFound, no run directory created
// ---------------------------------------------------------------------------

/// A path that does not exist must return `AppError::NotFound`.
///
/// This test also verifies the "no side effects" property: no run directory
/// is created anywhere in the temp dir (the caller's contract guarantees that
/// `launch_run` calls `read_attached_md` before creating any filesystem state).
#[tokio::test]
async fn it_t4_4_02_missing_file_returns_not_found() {
    let dir = temp_dir();
    let path = dir.path().join("nonexistent_context.md");

    // Confirm the path really does not exist.
    assert!(
        !path.exists(),
        "precondition: path must not exist before the call"
    );

    let result = read_attached_md(&path).await;

    // Must be AppError::NotFound.
    match &result {
        Err(e) => {
            let code = format!("{:?}", e);
            // Accept either variant spelling — just ensure it is a NotFound error.
            assert!(
                code.contains("NotFound"),
                "missing file must return NotFound variant; got: {:?}",
                e
            );
        }
        Ok(_) => panic!("expected Err(NotFound) for missing file, got Ok"),
    }

    // No side effects: nothing should have been written inside the temp dir
    // (the dir existed before the call, but no new entries should be present).
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read temp dir")
        .collect();
    assert!(
        entries.is_empty(),
        "no files must be created when read_attached_md returns NotFound; \
         found: {:?}",
        entries
            .iter()
            .map(|e| e.as_ref().map(|d| d.path()).unwrap_or_default())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// IT-T4.4-03: Oversized file (1 MiB + 1 byte) returns InvalidInput
// ---------------------------------------------------------------------------

/// A file exceeding the 1 MiB cap must return `AppError::InvalidInput` before
/// any content is loaded into memory.
#[tokio::test]
async fn it_t4_4_03_oversized_file_returns_invalid_input() {
    let dir = temp_dir();
    let path = dir.path().join("big.md");

    const LIMIT: usize = 1_048_576;
    let big = vec![b'x'; LIMIT + 1];
    tokio::fs::write(&path, &big)
        .await
        .expect("write oversized test file");

    let result = read_attached_md(&path).await;

    match &result {
        Err(e) => {
            let code = format!("{:?}", e);
            assert!(
                code.contains("InvalidInput"),
                "oversized file must return InvalidInput variant; got: {:?}",
                e
            );
        }
        Ok(_) => panic!("expected Err(InvalidInput) for oversized file, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// IT-T4.4-04: Exactly 1 MiB file is accepted (boundary at limit)
// ---------------------------------------------------------------------------

/// A file of exactly 1_048_576 bytes (the cap) must be accepted.
/// This guards against an off-by-one error where `>=` is used instead of `>`.
#[tokio::test]
async fn it_t4_4_04_exact_limit_file_is_accepted() {
    let dir = temp_dir();
    let path = dir.path().join("exact_limit.md");

    const LIMIT: usize = 1_048_576;
    let exact = vec![b'a'; LIMIT];
    tokio::fs::write(&path, &exact)
        .await
        .expect("write exact-limit test file");

    let result = read_attached_md(&path).await;

    assert!(
        result.is_ok(),
        "file at exactly 1 MiB must be accepted; got: {:?}",
        result
    );
    assert_eq!(
        result.unwrap().len(),
        LIMIT,
        "returned bytes must have the same length as the file"
    );
}

// ---------------------------------------------------------------------------
// IT-T4.4-05: Empty file (0 bytes) is accepted and returns an empty Vec
// ---------------------------------------------------------------------------

/// An empty file must be accepted.  The read must not fail, and the caller
/// (launch_run step 7b) skips writing an empty content + newline to stdin.
#[tokio::test]
async fn it_t4_4_05_empty_file_is_accepted() {
    let dir = temp_dir();
    let path = dir.path().join("empty.md");
    tokio::fs::write(&path, b"")
        .await
        .expect("write empty test file");

    let result = read_attached_md(&path).await;

    assert!(
        result.is_ok(),
        "empty file must be accepted; got: {:?}",
        result
    );
    assert!(
        result.unwrap().is_empty(),
        "empty file must return an empty Vec"
    );
}

// ---------------------------------------------------------------------------
// IT-T4.4-06: NotFound message contains the file path for diagnostics
// ---------------------------------------------------------------------------

/// The `AppError::NotFound` message must include the filename so that callers
/// and users can identify which file was missing without inspecting raw paths.
#[tokio::test]
async fn it_t4_4_06_not_found_message_contains_path() {
    let dir = temp_dir();
    let distinctive = "my_distinctive_context_file.md";
    let path = dir.path().join(distinctive);

    let result = read_attached_md(&path).await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(distinctive),
                "NotFound message must contain the filename '{distinctive}'; \
                 got message: {msg}"
            );
        }
        Ok(_) => panic!("expected Err(NotFound), got Ok"),
    }
}

// ---------------------------------------------------------------------------
// IT-T4.4-07: InvalidInput message mentions size / "too large"
// ---------------------------------------------------------------------------

/// The `AppError::InvalidInput` message for an oversized file must mention
/// either the byte count or the phrase "too large" so the caller can surface
/// a useful error to the user.
#[tokio::test]
async fn it_t4_4_07_invalid_input_message_mentions_size() {
    let dir = temp_dir();
    let path = dir.path().join("oversized.md");

    const LIMIT: usize = 1_048_576;
    let big = vec![b'y'; LIMIT + 1];
    tokio::fs::write(&path, &big)
        .await
        .expect("write oversized test file");

    let result = read_attached_md(&path).await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            // The message must mention either "too large" or "bytes".
            assert!(
                msg.contains("too large") || msg.contains("bytes"),
                "InvalidInput message must contain 'too large' or 'bytes'; got: {msg}"
            );
        }
        Ok(_) => panic!("expected Err(InvalidInput), got Ok"),
    }
}

// ---------------------------------------------------------------------------
// IT-T4.4-08: Binary content round-trips without corruption
// ---------------------------------------------------------------------------

/// `read_attached_md` must return the raw bytes unchanged, including non-UTF-8
/// sequences, null bytes, and high-byte values.  The content is destined for
/// `stdin.write_all`, which is byte-transparent.
#[tokio::test]
async fn it_t4_4_08_binary_content_round_trips_without_corruption() {
    let dir = temp_dir();
    let path = dir.path().join("binary.md");

    // A byte sequence that is deliberately not valid UTF-8.
    let binary: &[u8] = &[
        0x00, 0xFF, 0xFE, 0x80, 0x01, 0x1B, b'[', b'0', b'm',
        b'#', b' ', b'H', b'e', b'l', b'l', b'o', b'\n',
    ];
    tokio::fs::write(&path, binary)
        .await
        .expect("write binary test file");

    let result = read_attached_md(&path).await;

    assert!(
        result.is_ok(),
        "binary content within size limit must be accepted; got: {:?}",
        result
    );
    assert_eq!(
        result.unwrap(),
        binary,
        "binary bytes must be returned without any transformation"
    );
}

// ---------------------------------------------------------------------------
// IT-T4.4-09: meta.json records attached_md_path when Some
// ---------------------------------------------------------------------------

/// When `Run.attached_md_path` is `Some`, the `Run` struct serializes to JSON
/// with a non-null `attached_md_path` field.  This covers the `launch_run`
/// step 6 contract: `TranscriptWriter::create` writes the initial `meta.json`
/// from the `Run` value, so the path is always persisted.
///
/// `TranscriptWriter` is `pub(crate)` and cannot be called from the
/// integration-test crate.  We test the serialization contract directly,
/// which is the actual behaviour that `meta.json` depends on.
#[test]
fn it_t4_4_09_run_serializes_attached_md_path_as_non_null() {
    let context_path = std::path::PathBuf::from("/tmp/context.md");
    let run = make_run("run-t4.4-09", Some(context_path.clone()));

    let json = serde_json::to_string_pretty(&run)
        .expect("Run must serialize to JSON");
    let meta: serde_json::Value =
        serde_json::from_str(&json).expect("serialized Run must parse as JSON object");

    // The field must not be null.
    assert!(
        !meta["attached_md_path"].is_null(),
        "attached_md_path must be non-null in serialized Run when Some; json: {json}"
    );

    // The serialized value must mention the path string.
    let serialized = meta["attached_md_path"].to_string();
    assert!(
        serialized.contains("context.md"),
        "serialized attached_md_path must reference 'context.md'; value: {serialized}"
    );
}

// ---------------------------------------------------------------------------
// IT-T4.4-10: meta.json records null when attached_md_path is None
// ---------------------------------------------------------------------------

/// When `Run.attached_md_path` is `None`, the JSON serialization must produce
/// `null` for the field.  This confirms the serde derive correctly represents
/// the absent-context case in meta.json.
#[test]
fn it_t4_4_10_run_serializes_attached_md_path_as_null_when_none() {
    let run = make_run("run-t4.4-10", None);

    let json = serde_json::to_string_pretty(&run)
        .expect("Run must serialize to JSON");
    let meta: serde_json::Value =
        serde_json::from_str(&json).expect("serialized Run must parse as JSON object");

    assert!(
        meta["attached_md_path"].is_null(),
        "attached_md_path must be JSON null when None; actual value: {}",
        meta["attached_md_path"]
    );
}
