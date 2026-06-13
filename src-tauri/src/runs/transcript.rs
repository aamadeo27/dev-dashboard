// TranscriptWriter — append-only JSONL writer

use std::path::Path;

use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};

use super::{Run, RunEvent};

/// Per-run writer that owns async file handles for `meta.json`,
/// `transcript.jsonl`, and `raw.log` under a given `run_dir`.
pub struct TranscriptWriter {
    // Retained for diagnostics/identification; not read on the happy path.
    #[allow(dead_code)]
    run_id: String,
    run_dir: std::path::PathBuf,
    transcript: tokio::io::BufWriter<tokio::fs::File>,
    raw: tokio::io::BufWriter<tokio::fs::File>,
}

impl TranscriptWriter {
    /// Create `run_dir` (including parents), write initial `meta.json`
    /// atomically, and open `transcript.jsonl` and `raw.log` for append
    /// (creating them if they do not exist).
    pub async fn create(
        run_id: &str,
        run_dir: &std::path::Path,
        initial_meta: &Run,
    ) -> AppResult<Self> {
        // Create the directory tree.
        tokio::fs::create_dir_all(run_dir).await?;

        // Write initial meta.json atomically.
        write_meta_atomic(run_dir, initial_meta).await?;

        // Open transcript.jsonl for append (create if absent).
        let transcript_path = run_dir.join("transcript.jsonl");
        let transcript_file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript_path)
            .await?;

        // Open raw.log for append (create if absent).
        let raw_path = run_dir.join("raw.log");
        let raw_file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&raw_path)
            .await?;

        Ok(Self {
            run_id: run_id.to_string(),
            run_dir: run_dir.to_path_buf(),
            transcript: tokio::io::BufWriter::new(transcript_file),
            raw: tokio::io::BufWriter::new(raw_file),
        })
    }

    /// Serialize `event` as compact JSON followed by `\n`, write to
    /// `transcript.jsonl`, and flush immediately so the event is durable
    /// even if the process crashes before `close()` is called.
    pub async fn append_event(&mut self, event: &RunEvent) -> AppResult<()> {
        let mut line = serde_json::to_string(event).map_err(|e| AppError::Parse(e.to_string()))?;
        line.push('\n');
        self.transcript.write_all(line.as_bytes()).await?;
        self.transcript.flush().await?;
        Ok(())
    }

    /// Serialize all events in `events` as JSONL lines in a single pass, then
    /// flush once.  Prefer this over calling `append_event` in a loop from the
    /// caller — it avoids repeated flush calls and keeps the write path batched.
    pub async fn append_events(&mut self, events: &[RunEvent]) -> AppResult<()> {
        for event in events {
            let mut line =
                serde_json::to_string(event).map_err(|e| AppError::Parse(e.to_string()))?;
            line.push('\n');
            self.transcript.write_all(line.as_bytes()).await?;
        }
        self.transcript.flush().await?;
        Ok(())
    }

    /// Append raw bytes to `raw.log`. No per-write flush — best-effort.
    pub async fn append_raw(&mut self, bytes: &[u8]) -> AppResult<()> {
        self.raw.write_all(bytes).await?;
        Ok(())
    }

    /// Serialize `run` as pretty JSON, write to `meta.json.tmp`, then rename
    /// over `meta.json` for an atomic update.
    pub async fn update_meta(&self, run: &Run) -> AppResult<()> {
        write_meta_atomic(&self.run_dir, run).await
    }

    /// Flush all handles. Called when the run terminates.
    pub async fn close(mut self) -> AppResult<()> {
        self.transcript.flush().await?;
        self.raw.flush().await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Write `run` as pretty JSON to `<run_dir>/meta.json` atomically using a
/// tmp-write + rename strategy. On rename failure, the tmp file is removed
/// (best-effort) before the error is propagated.
///
/// # Windows note
/// The file handle is closed inside a scoped block before the rename call
/// because Windows requires the file to be closed before it can be renamed.
async fn write_meta_atomic(run_dir: &Path, run: &Run) -> AppResult<()> {
    let meta_path = run_dir.join("meta.json");
    let tmp_path = run_dir.join("meta.json.tmp");

    let pretty = serde_json::to_string_pretty(run).map_err(|e| AppError::Parse(e.to_string()))?;

    {
        // close file handle before rename — required on Windows
        let mut tmp_file = tokio::fs::File::create(&tmp_path).await?;
        tmp_file.write_all(pretty.as_bytes()).await?;
        tmp_file.flush().await?;
    }

    if let Err(rename_err) = tokio::fs::rename(&tmp_path, &meta_path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await; // best-effort cleanup
        return Err(rename_err.into());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::RunStatus;

    /// Construct a minimal `Run` for use in tests.
    fn make_run(id: &str) -> Run {
        Run {
            id: id.to_string(),
            project_id: "test-project".to_string(),
            project_path: std::env::temp_dir().join("project"),
            sequence_name: "test-seq".to_string(),
            attached_md_path: None,
            started_at: chrono::Utc::now(),
            ended_at: None,
            status: RunStatus::Running,
            exit_code: None,
            pid: None,
            note: None,
        }
    }

    /// Construct a minimal `RunEvent` for use in tests.
    fn make_event(text: &str) -> RunEvent {
        RunEvent::AssistantText {
            text: text.to_string(),
            ts: chrono::Utc::now(),
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: create() creates run_dir and all three files
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn create_makes_dir_and_files() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-1");

        let run = make_run("run-1");
        let writer = TranscriptWriter::create("run-1", &run_dir, &run)
            .await
            .expect("create");

        assert!(run_dir.exists(), "run_dir should exist");
        assert!(run_dir.join("meta.json").exists(), "meta.json should exist");
        assert!(
            run_dir.join("transcript.jsonl").exists(),
            "transcript.jsonl should exist"
        );
        assert!(run_dir.join("raw.log").exists(), "raw.log should exist");

        writer.close().await.expect("close");
    }

    // -----------------------------------------------------------------------
    // Test 2: append_event writes valid JSON + newline; read back and parse
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_event_writes_valid_json_line() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-2");

        let run = make_run("run-2");
        let mut writer = TranscriptWriter::create("run-2", &run_dir, &run)
            .await
            .expect("create");

        let event = make_event("hello world");
        writer.append_event(&event).await.expect("append_event");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        // Must end with exactly one newline.
        assert!(contents.ends_with('\n'), "line must end with newline");

        // The single line must parse as a RunEvent.
        let line = contents.trim_end_matches('\n');
        let parsed: RunEvent = serde_json::from_str(line).expect("parse RunEvent");

        // Verify the text survived the round-trip.
        if let RunEvent::AssistantText { text, .. } = parsed {
            assert_eq!(text, "hello world");
        } else {
            panic!("expected AssistantText variant");
        }
    }

    // -----------------------------------------------------------------------
    // Test 3: update_meta atomically updates meta.json; fields round-trip
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn update_meta_round_trips_run_fields() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-3");

        let run = make_run("run-3");
        let writer = TranscriptWriter::create("run-3", &run_dir, &run)
            .await
            .expect("create");

        // Update with a modified run.
        let mut updated = make_run("run-3");
        updated.status = RunStatus::Completed;
        updated.exit_code = Some(0);
        updated.note = Some("finished cleanly".to_string());
        writer.update_meta(&updated).await.expect("update_meta");
        writer.close().await.expect("close");

        // tmp file must not exist after rename.
        assert!(
            !run_dir.join("meta.json.tmp").exists(),
            "tmp file should be gone after atomic rename"
        );

        // meta.json must parse back to a Run with the updated fields.
        let bytes = std::fs::read(run_dir.join("meta.json")).expect("read meta.json");
        let parsed: Run = serde_json::from_slice(&bytes).expect("parse Run");
        assert_eq!(parsed.id, "run-3");
        assert!(matches!(parsed.status, RunStatus::Completed));
        assert_eq!(parsed.exit_code, Some(0));
        assert_eq!(parsed.note.as_deref(), Some("finished cleanly"));
    }

    // -----------------------------------------------------------------------
    // Test 4: multiple append_event calls produce valid multi-line JSONL
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn multiple_events_produce_valid_jsonl() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-4");

        let run = make_run("run-4");
        let mut writer = TranscriptWriter::create("run-4", &run_dir, &run)
            .await
            .expect("create");

        let messages = ["alpha", "beta", "gamma"];
        for msg in &messages {
            writer
                .append_event(&make_event(msg))
                .await
                .expect("append_event");
        }
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        // Each non-empty line must independently parse as a RunEvent.
        let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();

        assert_eq!(
            lines.len(),
            messages.len(),
            "should have one line per event"
        );

        for (i, line) in lines.iter().enumerate() {
            let parsed: RunEvent =
                serde_json::from_str(line).expect("each line must parse independently");
            if let RunEvent::AssistantText { text, .. } = parsed {
                assert_eq!(text, messages[i]);
            } else {
                panic!("expected AssistantText at line {}", i);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test 5: two writers in different run dirs don't interfere
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn two_writers_in_different_dirs_do_not_interfere() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir_a = tmp.path().join("run-a");
        let run_dir_b = tmp.path().join("run-b");

        let run_a = make_run("run-a");
        let run_b = make_run("run-b");

        let mut writer_a = TranscriptWriter::create("run-a", &run_dir_a, &run_a)
            .await
            .expect("create A");
        let mut writer_b = TranscriptWriter::create("run-b", &run_dir_b, &run_b)
            .await
            .expect("create B");

        writer_a
            .append_event(&make_event("from-a"))
            .await
            .expect("append A");
        writer_b
            .append_event(&make_event("from-b"))
            .await
            .expect("append B");

        writer_a.close().await.expect("close A");
        writer_b.close().await.expect("close B");

        // Each transcript must contain only its own event.
        let contents_a =
            std::fs::read_to_string(run_dir_a.join("transcript.jsonl")).expect("read A");
        let contents_b =
            std::fs::read_to_string(run_dir_b.join("transcript.jsonl")).expect("read B");

        assert!(
            contents_a.contains("from-a"),
            "A's transcript should contain 'from-a'"
        );
        assert!(
            !contents_a.contains("from-b"),
            "A's transcript must NOT contain 'from-b'"
        );

        assert!(
            contents_b.contains("from-b"),
            "B's transcript should contain 'from-b'"
        );
        assert!(
            !contents_b.contains("from-a"),
            "B's transcript must NOT contain 'from-a'"
        );

        // meta.json in each dir must parse to the correct run id.
        let meta_a: Run = serde_json::from_str(
            &std::fs::read_to_string(run_dir_a.join("meta.json")).expect("read meta A"),
        )
        .expect("parse meta A");
        let meta_b: Run = serde_json::from_str(
            &std::fs::read_to_string(run_dir_b.join("meta.json")).expect("read meta B"),
        )
        .expect("parse meta B");

        assert_eq!(meta_a.id, "run-a");
        assert_eq!(meta_b.id, "run-b");
    }

    // -----------------------------------------------------------------------
    // Test 6: append_raw writes bytes that are readable from raw.log
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_raw_writes_bytes_to_raw_log() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-6");

        let run = make_run("run-6");
        let mut writer = TranscriptWriter::create("run-6", &run_dir, &run)
            .await
            .expect("create");

        let chunk1 = b"stdout line 1\n";
        let chunk2 = b"stderr: warning\n";
        writer.append_raw(chunk1).await.expect("append_raw chunk1");
        writer.append_raw(chunk2).await.expect("append_raw chunk2");
        writer.close().await.expect("close");

        let bytes = std::fs::read(run_dir.join("raw.log")).expect("read raw.log");
        let mut expected = Vec::new();
        expected.extend_from_slice(chunk1);
        expected.extend_from_slice(chunk2);
        assert_eq!(
            bytes, expected,
            "raw.log must contain both chunks concatenated"
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: append_raw with binary/non-UTF-8 bytes round-trips correctly
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_raw_handles_binary_bytes() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-7");

        let run = make_run("run-7");
        let mut writer = TranscriptWriter::create("run-7", &run_dir, &run)
            .await
            .expect("create");

        // A byte sequence that is not valid UTF-8.
        let binary: &[u8] = &[0x00, 0xFF, 0xFE, 0x80, 0x01, 0x1B, b'[', b'0', b'm'];
        writer.append_raw(binary).await.expect("append_raw binary");
        writer.close().await.expect("close");

        let bytes = std::fs::read(run_dir.join("raw.log")).expect("read raw.log");
        assert_eq!(bytes, binary, "raw.log must preserve binary bytes exactly");
    }

    // -----------------------------------------------------------------------
    // Test 8: close() with zero events leaves transcript.jsonl empty (no panic)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn close_with_zero_events_produces_empty_transcript() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-8");

        let run = make_run("run-8");
        let writer = TranscriptWriter::create("run-8", &run_dir, &run)
            .await
            .expect("create");

        // Immediately close without appending anything.
        writer
            .close()
            .await
            .expect("close with zero events must not error");

        let transcript_path = run_dir.join("transcript.jsonl");
        assert!(
            transcript_path.exists(),
            "transcript.jsonl must still exist"
        );

        let contents = std::fs::read(transcript_path).expect("read transcript.jsonl");
        assert!(
            contents.is_empty(),
            "transcript.jsonl must be empty (0 bytes)"
        );
    }

    // -----------------------------------------------------------------------
    // Test 9: update_meta called twice — second call overwrites, not appends
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn update_meta_called_twice_overwrites_not_appends() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-9");

        let run = make_run("run-9");
        let writer = TranscriptWriter::create("run-9", &run_dir, &run)
            .await
            .expect("create");

        // First update: set status to Running with a note.
        let mut first = make_run("run-9");
        first.status = RunStatus::Running;
        first.note = Some("first update".to_string());
        writer.update_meta(&first).await.expect("first update_meta");

        // Second update: set status to Completed; note changes.
        let mut second = make_run("run-9");
        second.status = RunStatus::Completed;
        second.exit_code = Some(0);
        second.note = Some("second update".to_string());
        writer
            .update_meta(&second)
            .await
            .expect("second update_meta");

        writer.close().await.expect("close");

        // meta.json.tmp must not exist.
        assert!(
            !run_dir.join("meta.json.tmp").exists(),
            "meta.json.tmp must be gone after second rename"
        );

        // meta.json must reflect the second call only.
        let bytes = std::fs::read(run_dir.join("meta.json")).expect("read meta.json");
        let parsed: Run = serde_json::from_slice(&bytes).expect("parse Run");
        assert!(
            matches!(parsed.status, RunStatus::Completed),
            "status must be Completed after second update"
        );
        assert_eq!(parsed.exit_code, Some(0));
        assert_eq!(
            parsed.note.as_deref(),
            Some("second update"),
            "note must reflect second call, not first"
        );

        // The serialized bytes must not contain "first update" anywhere —
        // proving this is an overwrite, not an append.
        let text = std::str::from_utf8(&bytes).expect("meta.json is valid UTF-8");
        assert!(
            !text.contains("first update"),
            "meta.json must not contain text from the first update call"
        );
    }

    // -----------------------------------------------------------------------
    // Test 10: RunEvent::System round-trips correctly through JSONL
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn system_event_round_trips_through_jsonl() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-10");

        let run = make_run("run-10");
        let mut writer = TranscriptWriter::create("run-10", &run_dir, &run)
            .await
            .expect("create");

        let system_event = RunEvent::System {
            text: "Session started".to_string(),
            ts: chrono::Utc::now(),
        };
        writer
            .append_event(&system_event)
            .await
            .expect("append System event");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        let line = contents.trim_end_matches('\n');
        let parsed: RunEvent = serde_json::from_str(line).expect("parse RunEvent");

        match parsed {
            RunEvent::System { text, .. } => {
                assert_eq!(text, "Session started", "System event text must round-trip");
            }
            other => panic!("expected RunEvent::System, got {:?}", other),
        }

        // The JSON line must carry the correct type tag.
        assert!(
            line.contains("\"type\":\"system\""),
            "JSONL line must contain type tag 'system'"
        );
    }

    // -----------------------------------------------------------------------
    // Test 11: RunEvent::ToolCall (with serde_json::Value input) round-trips
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn tool_call_event_round_trips_through_jsonl() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-11");

        let run = make_run("run-11");
        let mut writer = TranscriptWriter::create("run-11", &run_dir, &run)
            .await
            .expect("create");

        let input_value = serde_json::json!({
            "path": "/tmp/file.txt",
            "content": "hello",
            "count": 42
        });
        let tool_call = RunEvent::ToolCall {
            id: "call-abc123".to_string(),
            name: "write_file".to_string(),
            input: input_value.clone(),
            ts: chrono::Utc::now(),
        };
        writer
            .append_event(&tool_call)
            .await
            .expect("append ToolCall");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        let line = contents.trim_end_matches('\n');
        let parsed: RunEvent = serde_json::from_str(line).expect("parse RunEvent");

        match parsed {
            RunEvent::ToolCall {
                id, name, input, ..
            } => {
                assert_eq!(id, "call-abc123");
                assert_eq!(name, "write_file");
                assert_eq!(input, input_value, "ToolCall input Value must round-trip");
            }
            other => panic!("expected RunEvent::ToolCall, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test 12: RunEvent::ToolResult with is_error=true round-trips
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn tool_result_error_event_round_trips_through_jsonl() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-12");

        let run = make_run("run-12");
        let mut writer = TranscriptWriter::create("run-12", &run_dir, &run)
            .await
            .expect("create");

        let output_value = serde_json::json!({"error": "file not found", "code": 404});
        let tool_result = RunEvent::ToolResult {
            call_id: "call-abc123".to_string(),
            output: output_value.clone(),
            is_error: true,
            ts: chrono::Utc::now(),
        };
        writer
            .append_event(&tool_result)
            .await
            .expect("append ToolResult");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        let line = contents.trim_end_matches('\n');
        let parsed: RunEvent = serde_json::from_str(line).expect("parse RunEvent");

        match parsed {
            RunEvent::ToolResult {
                call_id,
                output,
                is_error,
                ..
            } => {
                assert_eq!(call_id, "call-abc123");
                assert_eq!(output, output_value);
                assert!(is_error, "is_error must be true after round-trip");
            }
            other => panic!("expected RunEvent::ToolResult, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test 13: RunEvent::StepFailed round-trips both step and message fields
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn step_failed_event_round_trips_through_jsonl() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-13");

        let run = make_run("run-13");
        let mut writer = TranscriptWriter::create("run-13", &run_dir, &run)
            .await
            .expect("create");

        let step_failed = RunEvent::StepFailed {
            step: "compile".to_string(),
            message: "linker error: undefined symbol".to_string(),
            ts: chrono::Utc::now(),
        };
        writer
            .append_event(&step_failed)
            .await
            .expect("append StepFailed");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        let line = contents.trim_end_matches('\n');
        let parsed: RunEvent = serde_json::from_str(line).expect("parse RunEvent");

        match parsed {
            RunEvent::StepFailed { step, message, .. } => {
                assert_eq!(step, "compile");
                assert_eq!(message, "linker error: undefined symbol");
            }
            other => panic!("expected RunEvent::StepFailed, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test 14: append_event with special characters does not embed raw newlines
    //          (critical for JSONL integrity — one logical event per physical line)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_event_special_chars_no_embedded_newlines_in_jsonl() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-14");

        let run = make_run("run-14");
        let mut writer = TranscriptWriter::create("run-14", &run_dir, &run)
            .await
            .expect("create");

        // A text field containing literal newlines, tabs, backslashes, and Unicode.
        // serde_json compact serialization MUST escape \n as \\n, \t as \\t, etc.
        let tricky_text = "line1\nline2\ttabbed\\backslash\u{1F4A5}boom\"quote";
        let event = make_event(tricky_text);
        writer
            .append_event(&event)
            .await
            .expect("append_event with special chars");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        // The file must contain exactly one physical line (the trailing \n is the
        // record separator; splitting on \n yields ["<data>", ""] — 2 items, not more).
        let physical_lines: Vec<&str> = contents.split('\n').collect();
        assert_eq!(
            physical_lines.len(),
            2,
            "one event must produce exactly one physical line; got {} parts: {:?}",
            physical_lines.len(),
            physical_lines
        );
        assert!(
            physical_lines[1].is_empty(),
            "last element after split must be empty (trailing newline)"
        );

        // The single line must parse back and preserve the original text.
        let line = physical_lines[0];
        let parsed: RunEvent = serde_json::from_str(line).expect("parse RunEvent");
        if let RunEvent::AssistantText { text, .. } = parsed {
            assert_eq!(
                text, tricky_text,
                "special characters must survive the JSONL round-trip"
            );
        } else {
            panic!("expected AssistantText variant");
        }
    }

    // -----------------------------------------------------------------------
    // Test 15: append_event with a very long text field — no embedded newlines
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_event_long_text_no_embedded_newlines() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-15");

        let run = make_run("run-15");
        let mut writer = TranscriptWriter::create("run-15", &run_dir, &run)
            .await
            .expect("create");

        // 10 000 characters with embedded newlines every 80 chars.
        let long_text: String = (0..125)
            .map(|i| format!("{:079}\n", i)) // 79 digits + '\n' = 80 chars each
            .collect();
        assert_eq!(long_text.len(), 10_000);

        let event = make_event(&long_text);
        writer
            .append_event(&event)
            .await
            .expect("append_event long text");
        writer.close().await.expect("close");

        let raw_bytes =
            std::fs::read(run_dir.join("transcript.jsonl")).expect("read transcript.jsonl");
        let contents = String::from_utf8(raw_bytes).expect("transcript.jsonl must be UTF-8");

        // Must still be exactly one physical line.
        let physical_lines: Vec<&str> = contents.split('\n').collect();
        assert_eq!(
            physical_lines.len(),
            2,
            "long text with embedded newlines must still produce one JSONL line"
        );

        // Must round-trip.
        let parsed: RunEvent =
            serde_json::from_str(physical_lines[0]).expect("parse long-text event");
        if let RunEvent::AssistantText { text, .. } = parsed {
            assert_eq!(text, long_text, "long text must survive round-trip");
        } else {
            panic!("expected AssistantText");
        }
    }

    // -----------------------------------------------------------------------
    // Test 16: mixed event variants in a single transcript all parse correctly
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn mixed_event_variants_in_one_transcript() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-16");

        let run = make_run("run-16");
        let mut writer = TranscriptWriter::create("run-16", &run_dir, &run)
            .await
            .expect("create");

        let events: Vec<RunEvent> = vec![
            RunEvent::UserInput {
                text: "please run the tests".to_string(),
                ts: chrono::Utc::now(),
            },
            RunEvent::Thinking {
                text: "I should run cargo test".to_string(),
                ts: chrono::Utc::now(),
            },
            RunEvent::ToolCall {
                id: "tc-1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"command": "cargo test"}),
                ts: chrono::Utc::now(),
            },
            RunEvent::ToolResult {
                call_id: "tc-1".to_string(),
                output: serde_json::json!({"stdout": "test result: ok"}),
                is_error: false,
                ts: chrono::Utc::now(),
            },
            RunEvent::AssistantText {
                text: "All tests passed.".to_string(),
                ts: chrono::Utc::now(),
            },
        ];

        for ev in &events {
            writer.append_event(ev).await.expect("append mixed event");
        }
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(
            lines.len(),
            events.len(),
            "must have one line per event regardless of variant"
        );

        // Every line must independently parse as a RunEvent.
        for (i, line) in lines.iter().enumerate() {
            serde_json::from_str::<RunEvent>(line)
                .unwrap_or_else(|e| panic!("line {} failed to parse: {}", i, e));
        }

        // Spot-check the type tags in order.
        let expected_types = [
            "\"user_input\"",
            "\"thinking\"",
            "\"tool_call\"",
            "\"tool_result\"",
            "\"assistant_text\"",
        ];
        for (i, (line, expected_type)) in lines.iter().zip(expected_types.iter()).enumerate() {
            assert!(
                line.contains(expected_type),
                "line {} must contain type tag {}; got: {}",
                i,
                expected_type,
                line
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 17: append_events writes all events in a single batch; each line
    //          parses independently and order is preserved (FIX-ML-1)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_events_batch_writes_all_events_in_order() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-17");

        let run = make_run("run-17");
        let mut writer = TranscriptWriter::create("run-17", &run_dir, &run)
            .await
            .expect("create");

        let events: Vec<RunEvent> = vec![
            make_event("first"),
            make_event("second"),
            make_event("third"),
        ];
        writer.append_events(&events).await.expect("append_events");
        writer.close().await.expect("close");

        let contents = std::fs::read_to_string(run_dir.join("transcript.jsonl"))
            .expect("read transcript.jsonl");

        let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3, "must have exactly 3 lines");

        // Each line must parse as an AssistantText event with the correct text.
        let expected_texts = ["first", "second", "third"];
        for (i, (line, expected_text)) in lines.iter().zip(expected_texts.iter()).enumerate() {
            let parsed: RunEvent = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("line {} failed to parse: {}", i, e));
            if let RunEvent::AssistantText { text, .. } = parsed {
                assert_eq!(
                    text, *expected_text,
                    "line {} text must match; expected {:?}, got {:?}",
                    i, expected_text, text
                );
            } else {
                panic!("expected AssistantText at line {}", i);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test 18: append_events with an empty slice is a no-op — transcript
    //          remains empty and no error is returned (FIX-ML-1)
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn append_events_empty_slice_is_no_op() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let run_dir = tmp.path().join("run-18");

        let run = make_run("run-18");
        let mut writer = TranscriptWriter::create("run-18", &run_dir, &run)
            .await
            .expect("create");

        // Call append_events with an empty slice — must not error.
        writer
            .append_events(&[])
            .await
            .expect("append_events empty slice must not error");
        writer.close().await.expect("close");

        let bytes = std::fs::read(run_dir.join("transcript.jsonl")).expect("read transcript.jsonl");
        assert!(
            bytes.is_empty(),
            "transcript.jsonl must be empty after appending zero events"
        );
    }
}
