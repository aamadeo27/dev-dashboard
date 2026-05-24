// EventParser — heuristic stdout parser

pub mod patterns;

use chrono::Utc;
use super::RunEvent;

// ---------------------------------------------------------------------------
// Module-level constants (security guards)
// ---------------------------------------------------------------------------

/// Maximum bytes in a single line before it is truncated with a System event.
const MAX_LINE_BYTES: usize = 1_048_576; // 1 MiB

/// Maximum lines in an accumulator block before it is truncated.
const MAX_BLOCK_LINES: usize = 10_000;

/// Maximum bytes in an accumulator block before it is truncated.
const MAX_BLOCK_BYTES: usize = 4_194_304; // 4 MiB

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// Internal accumulation state of the parser.
#[derive(Debug)]
enum ParserState {
    /// Not inside any recognized block; each complete line is classified fresh.
    Idle,

    /// Accumulating a `ToolCall` block.
    ///
    /// Lines arrive indented (leading whitespace) and are collected as the raw
    /// input body.  The block ends on an empty line or a non-indented line.
    AccumulatingToolCall {
        name: String,
        input_lines: Vec<String>,
    },

    /// Accumulating a `Thinking` block.
    ///
    /// Opened by `<thinking>` or `✻ Thinking`.  Closed by `</thinking>` or an
    /// empty / non-indented line (for the alt-marker variant).
    AccumulatingThinking {
        lines: Vec<String>,
        /// `true` when opened with `<thinking>` — requires explicit close tag.
        needs_close_tag: bool,
    },

    /// Accumulating a `ToolResult` block.
    ///
    /// Opened by a `⎿` prefix or `Result:` keyword line.  Ends on empty line
    /// or non-indented line.
    AccumulatingToolResult {
        lines: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// Module-level helpers
// ---------------------------------------------------------------------------

/// Returns true if the line starts with a space or tab (indented block body).
#[inline]
fn is_indented(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Strip C0 control characters (except tab, LF, CR) and C1 control characters
/// from user-data strings to prevent terminal injection and display corruption.
fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            // Keep printable, tab (0x09), newline (0x0A), carriage return (0x0D).
            // Strip C0 (0x00–0x08, 0x0B–0x0C, 0x0E–0x1F) and C1 (0x80–0x9F).
            let cp = c as u32;
            !(cp <= 0x1F && cp != 0x09 && cp != 0x0A && cp != 0x0D)
            && !(cp >= 0x80 && cp <= 0x9F)
        })
        .collect()
}

/// Extract the file path from `--- a/<path>` or `+++ b/<path>` diff header lines.
fn extract_diff_path(input_lines: &[String]) -> String {
    for line in input_lines {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("--- a/") {
            return rest.to_owned();
        }
        if let Some(rest) = trimmed.strip_prefix("+++ b/") {
            return rest.to_owned();
        }
        // Handle "--- /dev/null" for new files — fall through.
    }
    String::new() // unknown path
}

/// Re-dispatch a line through Idle logic using a temporary parser instance.
/// Returns the new state produced by `process_idle_line`, or `None` if the
/// line did not trigger a state transition.
fn dispatch_idle_line(line: &str, events: &mut Vec<RunEvent>) -> Option<ParserState> {
    let mut tmp = EventParser::new_no_log();
    tmp.process_idle_line(line, events)
}

// ---------------------------------------------------------------------------
// EventParser
// ---------------------------------------------------------------------------

/// Stateful, line-buffered, heuristic parser for Claude CLI stdout.
///
/// Feed arbitrary byte chunks via [`EventParser::feed`]; it buffers until
/// newlines and returns zero or more [`RunEvent`]s per call.
///
/// Design goals:
/// - No per-byte heap allocations in the hot path.
/// - Never panic or silently drop input; malformed blocks fall back to
///   [`RunEvent::System`].
pub struct EventParser {
    /// Incomplete-line carry buffer.  Reused across `feed()` calls.
    line_buf: Vec<u8>,
    /// Current accumulation state.
    state: ParserState,
}

impl EventParser {
    /// Create a new `EventParser` and log the active pattern version.
    pub fn new() -> Self {
        tracing::info!(
            pattern_version = patterns::PATTERN_VERSION,
            "EventParser initialized"
        );
        Self {
            line_buf: Vec::with_capacity(256),
            state: ParserState::Idle,
        }
    }

    /// Accept an arbitrary byte chunk and return any fully-parsed events.
    ///
    /// Bytes are appended to an internal line buffer.  Each time a `\n` is
    /// found the buffered line is decoded and processed.  `\r\n` sequences are
    /// handled by stripping the trailing `\r` before decoding.
    ///
    /// Allocation policy: `self.line_buf` is reused across calls (only a
    /// `clear()` on each complete line); no per-byte allocations occur.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<RunEvent> {
        let mut events = Vec::new();

        for &byte in bytes {
            if byte == b'\n' {
                // Strip optional CR from CRLF sequences.
                if self.line_buf.last() == Some(&b'\r') {
                    self.line_buf.pop();
                }

                // Decode the buffered line; treat invalid UTF-8 as a System event.
                let line = match std::str::from_utf8(&self.line_buf) {
                    Ok(s) => s.to_owned(),
                    Err(_) => {
                        let raw = String::from_utf8_lossy(&self.line_buf).into_owned();
                        events.push(RunEvent::System {
                            text: strip_control_chars(&raw),
                            ts: Utc::now(),
                        });
                        self.line_buf.clear();
                        continue;
                    }
                };

                self.line_buf.clear();
                self.process_line(&line, &mut events);
            } else {
                // FIX-1: Guard against unbounded line_buf growth.
                if self.line_buf.len() >= MAX_LINE_BYTES {
                    events.push(RunEvent::System {
                        text: format!("[parser] oversized line truncated at {} bytes", MAX_LINE_BYTES),
                        ts: Utc::now(),
                    });
                    self.line_buf.clear();
                    self.state = ParserState::Idle;
                    // Do not push the current byte — it belongs to a new (fresh) line.
                } else {
                    self.line_buf.push(byte);
                }
            }
        }

        events
    }

    // -----------------------------------------------------------------------
    // Line dispatch
    // -----------------------------------------------------------------------

    fn process_line(&mut self, line: &str, events: &mut Vec<RunEvent>) {
        // Check StepFailed sentinel first (currently never fires since sentinel is "").
        if !patterns::STEP_FAILED_SENTINEL.is_empty()
            && line == patterns::STEP_FAILED_SENTINEL
        {
            events.push(RunEvent::StepFailed {
                step: String::new(),
                message: line.to_owned(),
                ts: Utc::now(),
            });
            return;
        }

        // Dispatch based on current state.
        //
        // The Idle arm calls `process_idle_line` which takes `&mut self`.  We
        // must not hold a simultaneous `&mut self.state` reference, so we check
        // for Idle with a pattern guard first and fall through to the non-Idle
        // arms in the match below.
        if matches!(self.state, ParserState::Idle) {
            let new_state = self.process_idle_line(line, events);
            if let Some(s) = new_state {
                self.state = s;
            }
            return;
        }

        let new_state = match &mut self.state {
            ParserState::Idle => unreachable!("handled above"),
            ParserState::AccumulatingToolCall { name, input_lines } => {
                Self::process_tool_call_line(line, name, input_lines, events)
            }
            ParserState::AccumulatingThinking { lines, needs_close_tag } => {
                Self::process_thinking_line(line, lines, *needs_close_tag, events)
            }
            ParserState::AccumulatingToolResult { lines } => {
                Self::process_tool_result_line(line, lines, events)
            }
        };

        if let Some(s) = new_state {
            self.state = s;
        }
    }

    // -----------------------------------------------------------------------
    // State handlers — each returns Some(new_state) to transition, or None to
    // keep the current state.
    // -----------------------------------------------------------------------

    fn process_idle_line(&mut self, line: &str, events: &mut Vec<RunEvent>) -> Option<ParserState> {
        // ── Tool call opener ────────────────────────────────────────────────
        if line.starts_with(patterns::TOOL_CALL_PREFIX) {
            let name = line[patterns::TOOL_CALL_PREFIX.len()..].trim().to_owned();
            return Some(ParserState::AccumulatingToolCall {
                name,
                input_lines: Vec::new(),
            });
        }

        // ── Tool result opener ──────────────────────────────────────────────
        if line.starts_with(patterns::TOOL_RESULT_PREFIX)
            || line.starts_with(patterns::TOOL_RESULT_KEYWORD)
        {
            // The first line itself may carry content after the prefix.
            let content = if line.starts_with(patterns::TOOL_RESULT_PREFIX) {
                line[patterns::TOOL_RESULT_PREFIX.len()..].trim().to_owned()
            } else {
                // "Result: ..." — keep everything after the keyword
                line[patterns::TOOL_RESULT_KEYWORD.len()..].trim_start().to_owned()
            };
            let mut lines = Vec::new();
            if !content.is_empty() {
                lines.push(content);
            }
            return Some(ParserState::AccumulatingToolResult { lines });
        }

        // ── Thinking opener (XML tag) ───────────────────────────────────────
        if line.contains(patterns::THINKING_OPEN) {
            // Content may appear on the same line after the open tag.
            let after = line.splitn(2, patterns::THINKING_OPEN).nth(1).unwrap_or("");
            // If the close tag is also on this line, emit immediately.
            if after.contains(patterns::THINKING_CLOSE) {
                let text = after
                    .splitn(2, patterns::THINKING_CLOSE)
                    .next()
                    .unwrap_or("")
                    .to_owned();
                events.push(RunEvent::Thinking {
                    text: strip_control_chars(&text),
                    ts: Utc::now(),
                });
                return None;
            }
            let mut lines = Vec::new();
            if !after.is_empty() {
                lines.push(after.to_owned());
            }
            return Some(ParserState::AccumulatingThinking {
                lines,
                needs_close_tag: true,
            });
        }

        // ── Thinking opener (alt marker) ────────────────────────────────────
        if line.starts_with(patterns::THINKING_ALT_MARKER) {
            return Some(ParserState::AccumulatingThinking {
                lines: Vec::new(),
                needs_close_tag: false,
            });
        }

        // RunEvent::UserInput is not emitted here — Claude CLI stdout does not echo
        // user input lines. UserInput events are injected by the T4.3 caller (RunSession)
        // when it sends input to the process stdin.

        // ── Fallback: plain assistant text ──────────────────────────────────
        events.push(RunEvent::AssistantText {
            text: strip_control_chars(line),
            ts: Utc::now(),
        });
        None
    }

    /// Returns `Some(next_state)` when the block ends, else `None`.
    fn process_tool_call_line(
        line: &str,
        name: &mut String,
        input_lines: &mut Vec<String>,
        events: &mut Vec<RunEvent>,
    ) -> Option<ParserState> {
        let is_empty = line.is_empty();
        let indented = is_indented(line);

        // Block-end conditions.
        if is_empty || !indented {
            // Emit accumulated ToolCall (or transition to diff/FileEdit).
            let tool_name = name.clone();
            let raw_input: String = input_lines.join("\n");

            // Check if the tool is an edit/write tool AND has diff content.
            let is_edit = patterns::EDIT_TOOL_NAMES
                .iter()
                .any(|&n| n == tool_name.as_str());

            // Try to detect a diff block inside the accumulated lines.
            let has_diff_hunk = input_lines
                .iter()
                .any(|l| l.trim_start().starts_with(patterns::DIFF_HUNK_PREFIX));

            if is_edit && has_diff_hunk {
                // Emit a FileEdit event from accumulated diff lines.
                // Strip leading indentation before diff-line accounting.
                let diff_body: Vec<String> = input_lines
                    .iter()
                    .map(|l| l.trim_start().to_owned())
                    .filter(|l| l.starts_with(patterns::DIFF_HUNK_PREFIX)
                        || l.starts_with('+')
                        || l.starts_with('-'))
                    .collect();

                let (additions, deletions) = count_diff_lines(&diff_body);
                // FIX-9: move raw_input directly into diff (no clone needed).
                let diff = raw_input;

                events.push(RunEvent::FileEdit {
                    // FIX-5: extract actual file path from diff headers.
                    path: extract_diff_path(input_lines),
                    diff,
                    additions,
                    deletions,
                    ts: Utc::now(),
                });
            } else {
                // FIX-3: size guard before JSON parse.
                if raw_input.len() > MAX_BLOCK_BYTES {
                    events.push(RunEvent::System {
                        text: format!("[parser] tool input too large ({} bytes), skipped", raw_input.len()),
                        ts: Utc::now(),
                    });
                    if !is_empty {
                        return dispatch_idle_line(line, events).or(Some(ParserState::Idle));
                    }
                    return Some(ParserState::Idle);
                }

                // Parse input as JSON; fall back to System on error.
                let input_value: serde_json::Value =
                    match serde_json::from_str(&raw_input) {
                        Ok(v) => v,
                        Err(_) if raw_input.is_empty() => serde_json::Value::Null,
                        Err(_) => {
                            // Malformed block body → System event with raw content.
                            events.push(RunEvent::System {
                                text: strip_control_chars(&format!(
                                    "malformed tool input for '{}': {}",
                                    tool_name, raw_input
                                )),
                                ts: Utc::now(),
                            });
                            // Re-dispatch the non-empty triggering line in Idle so
                            // it is not silently dropped.
                            if !is_empty {
                                return dispatch_idle_line(line, events).or(Some(ParserState::Idle));
                            }
                            return Some(ParserState::Idle);
                        }
                    };

                events.push(RunEvent::ToolCall {
                    // FIX-6: use UUID v7 (time-ordered).
                    id: uuid::Uuid::now_v7().to_string(),
                    name: tool_name,
                    input: input_value,
                    ts: Utc::now(),
                });
            }

            // If we ended on a non-empty non-indented line, re-process it in Idle.
            // FIX-4: use dispatch_idle_line and propagate the returned state.
            if !is_empty {
                return dispatch_idle_line(line, events).or(Some(ParserState::Idle));
            }

            return Some(ParserState::Idle);
        }

        // FIX-2: guard against unbounded accumulator growth.
        let total_bytes: usize = input_lines.iter().map(|l| l.len()).sum();
        if input_lines.len() >= MAX_BLOCK_LINES || total_bytes + line.len() > MAX_BLOCK_BYTES {
            events.push(RunEvent::System {
                text: "[parser] block truncated: too many lines/bytes".to_owned(),
                ts: Utc::now(),
            });
            return Some(ParserState::Idle);
        }

        // Inside the block — accumulate the line.
        input_lines.push(line.to_owned());
        None
    }

    fn process_thinking_line(
        line: &str,
        lines: &mut Vec<String>,
        needs_close_tag: bool,
        events: &mut Vec<RunEvent>,
    ) -> Option<ParserState> {
        // Close by explicit tag.
        if line.contains(patterns::THINKING_CLOSE) {
            let before = line
                .splitn(2, patterns::THINKING_CLOSE)
                .next()
                .unwrap_or("");
            if !before.is_empty() {
                lines.push(before.to_owned());
            }
            let text = lines.join("\n");
            events.push(RunEvent::Thinking {
                text: strip_control_chars(&text),
                ts: Utc::now(),
            });
            return Some(ParserState::Idle);
        }

        // For alt-marker variant, close on empty or non-indented line.
        if !needs_close_tag {
            let is_empty = line.is_empty();
            let indented = is_indented(line);
            if is_empty || !indented {
                let text = lines.join("\n");
                events.push(RunEvent::Thinking {
                    text: strip_control_chars(&text),
                    ts: Utc::now(),
                });
                return Some(ParserState::Idle);
            }
        }

        // FIX-2: guard against unbounded accumulator growth.
        let total_bytes: usize = lines.iter().map(|l| l.len()).sum();
        if lines.len() >= MAX_BLOCK_LINES || total_bytes + line.len() > MAX_BLOCK_BYTES {
            events.push(RunEvent::System {
                text: "[parser] block truncated: too many lines/bytes".to_owned(),
                ts: Utc::now(),
            });
            return Some(ParserState::Idle);
        }

        lines.push(line.to_owned());
        None
    }

    fn process_tool_result_line(
        line: &str,
        lines: &mut Vec<String>,
        events: &mut Vec<RunEvent>,
    ) -> Option<ParserState> {
        let is_empty = line.is_empty();
        let indented = is_indented(line);

        if is_empty || !indented {
            let raw_output = lines.join("\n");

            // FIX-3: size guard before JSON parse.
            if raw_output.len() > MAX_BLOCK_BYTES {
                events.push(RunEvent::System {
                    text: format!("[parser] tool output too large ({} bytes), skipped", raw_output.len()),
                    ts: Utc::now(),
                });
                if !is_empty {
                    return dispatch_idle_line(line, events).or(Some(ParserState::Idle));
                }
                return Some(ParserState::Idle);
            }

            let output: serde_json::Value =
                serde_json::from_str(&raw_output).unwrap_or_else(|_| {
                    // FIX-7: strip control chars from plain-text fallback.
                    serde_json::Value::String(strip_control_chars(&raw_output))
                });

            let is_error = raw_output.to_lowercase().contains("error");

            events.push(RunEvent::ToolResult {
                // FIX-6: use UUID v7 (time-ordered).
                call_id: uuid::Uuid::now_v7().to_string(),
                output,
                is_error,
                ts: Utc::now(),
            });

            // Re-process a non-empty terminating line in Idle.
            // FIX-4: use dispatch_idle_line and propagate the returned state.
            if !is_empty {
                return dispatch_idle_line(line, events).or(Some(ParserState::Idle));
            }

            return Some(ParserState::Idle);
        }

        // FIX-2: guard against unbounded accumulator growth.
        let total_bytes: usize = lines.iter().map(|l| l.len()).sum();
        if lines.len() >= MAX_BLOCK_LINES || total_bytes + line.len() > MAX_BLOCK_BYTES {
            events.push(RunEvent::System {
                text: "[parser] block truncated: too many lines/bytes".to_owned(),
                ts: Utc::now(),
            });
            return Some(ParserState::Idle);
        }

        lines.push(line.to_owned());
        None
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Construct a parser without emitting the initialization log line.
    /// Used internally to redirect a line from a block-end context into Idle
    /// processing without creating spurious tracing events.
    fn new_no_log() -> Self {
        Self {
            line_buf: Vec::with_capacity(64),
            state: ParserState::Idle,
        }
    }
}

impl Default for EventParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Diff accounting helpers
// ---------------------------------------------------------------------------

/// Count added and deleted lines in a diff body.
///
/// Lines starting with `+` (but not `+++`) are additions;
/// lines starting with `-` (but not `---`) are deletions.
fn count_diff_lines(lines: &[String]) -> (u32, u32) {
    let mut additions: u32 = 0;
    let mut deletions: u32 = 0;
    for line in lines {
        if line.starts_with("+++") || line.starts_with("---") {
            // Header lines — skip.
            continue;
        }
        if line.starts_with('+') {
            additions += 1;
        } else if line.starts_with('-') {
            deletions += 1;
        }
    }
    (additions, deletions)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn parser() -> EventParser {
        EventParser::new_no_log()
    }

    fn feed_all(p: &mut EventParser, chunks: &[&[u8]]) -> Vec<RunEvent> {
        chunks.iter().flat_map(|c| p.feed(c)).collect()
    }

    // ── AC1: Single chunk, one event ─────────────────────────────────────────

    #[test]
    fn single_chunk_one_assistant_text_event() {
        let mut p = parser();
        let events = p.feed(b"Hello world\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            RunEvent::AssistantText { text, .. } => assert_eq!(text, "Hello world"),
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    // ── AC2: Event split across many chunks ──────────────────────────────────

    #[test]
    fn event_split_across_chunks_joins_correctly() {
        let mut p = parser();
        let events = feed_all(&mut p, &[b"Hello", b" world\n"]);
        assert_eq!(events.len(), 1, "expected exactly 1 event");
        match &events[0] {
            RunEvent::AssistantText { text, .. } => assert_eq!(text, "Hello world"),
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    #[test]
    fn event_split_across_many_one_byte_chunks() {
        let mut p = parser();
        let msg = b"streaming text\n";
        let events: Vec<RunEvent> = msg.iter().flat_map(|b| p.feed(std::slice::from_ref(b))).collect();
        assert_eq!(events.len(), 1);
        match &events[0] {
            RunEvent::AssistantText { text, .. } => assert_eq!(text, "streaming text"),
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    // ── AC3: Multiple events in one chunk ────────────────────────────────────

    #[test]
    fn multiple_events_in_one_chunk() {
        let mut p = parser();
        let events = p.feed(b"line one\nline two\n");
        assert_eq!(events.len(), 2);
        match (&events[0], &events[1]) {
            (RunEvent::AssistantText { text: t1, .. }, RunEvent::AssistantText { text: t2, .. }) => {
                assert_eq!(t1, "line one");
                assert_eq!(t2, "line two");
            }
            other => panic!("expected two AssistantText events, got {:?}", other),
        }
    }

    #[test]
    fn three_lines_produce_three_events() {
        let mut p = parser();
        let events = p.feed(b"a\nb\nc\n");
        assert_eq!(events.len(), 3);
        for ev in &events {
            assert!(matches!(ev, RunEvent::AssistantText { .. }));
        }
    }

    // ── AC4: Unknown line → AssistantText fallback ───────────────────────────

    #[test]
    fn unknown_line_falls_back_to_assistant_text() {
        let mut p = parser();
        let events = p.feed(b"some random unrecognised line\n");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], RunEvent::AssistantText { .. }));
    }

    #[test]
    fn empty_line_produces_assistant_text_event() {
        let mut p = parser();
        let events = p.feed(b"\n");
        // An empty line in Idle state is just AssistantText with empty string.
        assert_eq!(events.len(), 1);
        match &events[0] {
            RunEvent::AssistantText { text, .. } => assert_eq!(text, ""),
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    // ── AC5: Malformed block body → System event ─────────────────────────────

    #[test]
    fn malformed_tool_call_body_emits_system_event() {
        let mut p = parser();
        // Open a tool call block.
        let mut events = p.feed(b"\xE2\x8F\xBA Tool: Foo\n");
        // Feed a clearly non-JSON indented line.
        events.extend(p.feed(b"   NOT_JSON_AT_ALL\n"));
        // Terminate with a non-indented line to flush the block.
        events.extend(p.feed(b"next line\n"));

        // We should have a System event for the malformed input.
        let has_system = events.iter().any(|e| matches!(e, RunEvent::System { .. }));
        assert!(has_system, "expected a System event for malformed tool body; got {:?}", events);
    }

    #[test]
    fn tool_call_with_valid_json_emits_tool_call_event() {
        let mut p = parser();
        let mut events = p.feed(b"\xE2\x8F\xBA Tool: Read\n");
        events.extend(p.feed(b"  {\"path\": \"/tmp/foo\"}\n"));
        events.extend(p.feed(b"\n")); // empty line closes the block

        let tool_calls: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolCall { .. }))
            .collect();
        assert_eq!(tool_calls.len(), 1, "expected one ToolCall event; got {:?}", events);
        match &tool_calls[0] {
            RunEvent::ToolCall { name, input, .. } => {
                assert_eq!(name, "Read");
                assert_eq!(input["path"], "/tmp/foo");
            }
            _ => unreachable!(),
        }
    }

    // ── Thinking events ───────────────────────────────────────────────────────

    #[test]
    fn thinking_xml_block_emits_thinking_event() {
        let mut p = parser();
        let mut events = p.feed(b"<thinking>\n");
        events.extend(p.feed(b"  deep thought\n"));
        events.extend(p.feed(b"</thinking>\n"));

        let thinking: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Thinking { .. }))
            .collect();
        assert_eq!(thinking.len(), 1, "expected one Thinking event; got {:?}", events);
        match &thinking[0] {
            RunEvent::Thinking { text, .. } => assert!(text.contains("deep thought")),
            _ => unreachable!(),
        }
    }

    #[test]
    fn thinking_alt_marker_emits_thinking_event() {
        let mut p = parser();
        // ✻ Thinking marker (U+2733)
        let mut events = p.feed("\u{2733} Thinking\n".as_bytes());
        events.extend(p.feed(b"  some thought\n"));
        events.extend(p.feed(b"\n")); // empty line closes alt-thinking block

        let thinking: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Thinking { .. }))
            .collect();
        assert_eq!(thinking.len(), 1, "expected one Thinking event; got {:?}", events);
    }

    // ── CRLF handling ─────────────────────────────────────────────────────────

    #[test]
    fn crlf_line_endings_are_handled() {
        let mut p = parser();
        let events = p.feed(b"hello\r\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            RunEvent::AssistantText { text, .. } => assert_eq!(text, "hello"),
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    // ── StepFailed sentinel (currently never fires) ───────────────────────────

    #[test]
    fn step_failed_sentinel_empty_means_never_fires() {
        // The sentinel is currently "", so it never matches any real line.
        assert_eq!(patterns::STEP_FAILED_SENTINEL, "", "sentinel must be empty per T4.8");

        let mut p = parser();
        // Feed a line that could be mistaken for the sentinel if it were non-empty.
        let events = p.feed(b"some line\n");
        let has_step_failed = events.iter().any(|e| matches!(e, RunEvent::StepFailed { .. }));
        assert!(!has_step_failed, "StepFailed must not fire while sentinel is empty");
    }

    #[test]
    fn empty_line_does_not_trigger_step_failed() {
        // Even an empty line (which equals the empty sentinel) should NOT fire
        // StepFailed — the guard `!STEP_FAILED_SENTINEL.is_empty()` prevents it.
        let mut p = parser();
        let events = p.feed(b"\n");
        let has_step_failed = events.iter().any(|e| matches!(e, RunEvent::StepFailed { .. }));
        assert!(!has_step_failed, "empty sentinel must never match");
    }

    // ── Pattern version constant ──────────────────────────────────────────────

    #[test]
    fn pattern_version_constant_is_set() {
        assert!(!patterns::PATTERN_VERSION.is_empty());
        assert_eq!(patterns::PATTERN_VERSION, "v1-heuristic-2026-05-23");
    }

    // ── Diff / FileEdit ───────────────────────────────────────────────────────

    #[test]
    fn count_diff_lines_ignores_headers() {
        let lines: Vec<String> = vec![
            "--- a/foo.rs".into(),
            "+++ b/foo.rs".into(),
            "@@ -1,3 +1,4 @@".into(),
            "+new line".into(),
            "-old line".into(),
            " context".into(),
        ];
        let (additions, deletions) = count_diff_lines(&lines);
        assert_eq!(additions, 1);
        assert_eq!(deletions, 1);
    }

    #[test]
    fn edit_tool_with_diff_emits_file_edit_event() {
        let mut p = parser();
        // ⏺ Tool: Edit
        let mut events = p.feed(b"\xE2\x8F\xBA Tool: Edit\n");
        events.extend(p.feed(b"  @@ -1,3 +1,4 @@\n"));
        events.extend(p.feed(b"  +added line\n"));
        events.extend(p.feed(b"  -removed line\n"));
        events.extend(p.feed(b"\n")); // close block

        let file_edits: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::FileEdit { .. }))
            .collect();
        assert_eq!(file_edits.len(), 1, "expected one FileEdit event; got {:?}", events);
        match &file_edits[0] {
            RunEvent::FileEdit { additions, deletions, .. } => {
                // additions/deletions counted from the diff body
                assert_eq!(*additions, 1, "expected 1 addition");
                assert_eq!(*deletions, 1, "expected 1 deletion");
            }
            _ => unreachable!(),
        }
    }

    // ── Tool result ───────────────────────────────────────────────────────────

    #[test]
    fn tool_result_prefix_emits_tool_result_event() {
        let mut p = parser();
        // ⎿ prefix (U+2380)
        let mut events = p.feed("\u{2380} {\"result\": \"ok\"}\n".as_bytes());
        events.extend(p.feed(b"\n")); // close block

        let results: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolResult { .. }))
            .collect();
        assert_eq!(results.len(), 1, "expected one ToolResult event; got {:?}", events);
    }

    #[test]
    fn result_keyword_opens_tool_result_block() {
        let mut p = parser();
        let mut events = p.feed(b"Result: some output\n");
        events.extend(p.feed(b"\n"));

        let results: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolResult { .. }))
            .collect();
        assert_eq!(results.len(), 1, "expected one ToolResult event; got {:?}", events);
    }

    // ── No partial events on incomplete lines ─────────────────────────────────

    #[test]
    fn no_events_emitted_for_incomplete_line() {
        let mut p = parser();
        let events = p.feed(b"no newline here");
        assert!(events.is_empty(), "should not emit until newline; got {:?}", events);
    }

    #[test]
    fn incomplete_then_completed_across_feeds() {
        let mut p = parser();
        let e1 = p.feed(b"part one ");
        let e2 = p.feed(b"part two\n");
        assert!(e1.is_empty());
        assert_eq!(e2.len(), 1);
        match &e2[0] {
            RunEvent::AssistantText { text, .. } => assert_eq!(text, "part one part two"),
            other => panic!("got {:?}", other),
        }
    }

    // ── GAP: Thinking XML — multi-line body is joined ────────────────────────

    /// A thinking block opened with `<thinking>` and closed with `</thinking>`
    /// that spans several lines must join all interior lines with `\n`.
    #[test]
    fn thinking_xml_multiline_body_is_joined() {
        let mut p = parser();
        let mut events = p.feed(b"<thinking>\n");
        events.extend(p.feed(b"  line one\n"));
        events.extend(p.feed(b"  line two\n"));
        events.extend(p.feed(b"  line three\n"));
        events.extend(p.feed(b"</thinking>\n"));

        let thinking: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Thinking { .. }))
            .collect();
        assert_eq!(thinking.len(), 1, "expected exactly one Thinking event; got {:?}", events);
        match &thinking[0] {
            RunEvent::Thinking { text, .. } => {
                assert!(text.contains("line one"), "text missing 'line one': {:?}", text);
                assert!(text.contains("line two"), "text missing 'line two': {:?}", text);
                assert!(text.contains("line three"), "text missing 'line three': {:?}", text);
                // All three lines must be present in order, separated by newlines.
                assert!(text.contains("line one\n  line two\n  line three"),
                    "lines not joined correctly: {:?}", text);
            }
            _ => unreachable!(),
        }
    }

    // ── GAP: Thinking alt-marker — closed by a non-indented line ────────────

    /// The alt-marker thinking block (`✻ Thinking`) must close when it
    /// encounters any non-indented, non-empty line — not only an empty line.
    /// The closing non-indented line itself is NOT re-dispatched (it is dropped
    /// by the state machine), so no additional events should appear for it.
    #[test]
    fn thinking_alt_marker_closes_on_non_indented_line() {
        let mut p = parser();
        let mut events = p.feed("\u{2733} Thinking\n".as_bytes());
        events.extend(p.feed(b"  indented thought\n"));
        // A non-indented, non-empty line closes the block.
        // This line is dropped (not re-dispatched) per current state-machine behaviour.
        events.extend(p.feed(b"non-indented continuation\n"));

        let thinking: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Thinking { .. }))
            .collect();
        assert_eq!(thinking.len(), 1, "expected one Thinking event; got {:?}", events);
        match &thinking[0] {
            RunEvent::Thinking { text, .. } => {
                assert!(text.contains("indented thought"), "body not captured: {:?}", text);
            }
            _ => unreachable!(),
        }
    }

    // ── GAP: ToolResult — multi-line body is joined ──────────────────────────

    /// A tool-result block with several indented continuation lines must join
    /// them all into the output value.
    #[test]
    fn tool_result_multiline_body_is_joined() {
        let mut p = parser();
        // Open with the ⎿ prefix (no inline content).
        let mut events = p.feed("\u{2380}\n".as_bytes());
        events.extend(p.feed(b"  first line\n"));
        events.extend(p.feed(b"  second line\n"));
        events.extend(p.feed(b"  third line\n"));
        events.extend(p.feed(b"\n")); // empty line closes block

        let results: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolResult { .. }))
            .collect();
        assert_eq!(results.len(), 1, "expected one ToolResult event; got {:?}", events);
        match &results[0] {
            RunEvent::ToolResult { output, .. } => {
                // The body is plain text (not JSON), so output is a JSON string.
                let as_str = output.as_str().expect("output should be a JSON string");
                assert!(as_str.contains("first line"), "missing 'first line' in {:?}", as_str);
                assert!(as_str.contains("second line"), "missing 'second line' in {:?}", as_str);
                assert!(as_str.contains("third line"), "missing 'third line' in {:?}", as_str);
            }
            _ => unreachable!(),
        }
    }

    // ── GAP: Write tool triggers FileEdit ────────────────────────────────────

    /// `Write` must be treated the same as `Edit` — a diff block inside a
    /// `⏺ Tool: Write` header must produce a `FileEdit` event, not a `ToolCall`.
    #[test]
    fn write_tool_with_diff_emits_file_edit_event() {
        let mut p = parser();
        // ⏺ Tool: Write
        let mut events = p.feed(b"\xE2\x8F\xBA Tool: Write\n");
        events.extend(p.feed(b"  @@ -0,0 +1,2 @@\n"));
        events.extend(p.feed(b"  +first new line\n"));
        events.extend(p.feed(b"  +second new line\n"));
        events.extend(p.feed(b"\n")); // close block

        let file_edits: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::FileEdit { .. }))
            .collect();
        assert_eq!(file_edits.len(), 1, "expected one FileEdit event for Write tool; got {:?}", events);
        match &file_edits[0] {
            RunEvent::FileEdit { additions, deletions, .. } => {
                assert_eq!(*additions, 2, "expected 2 additions");
                assert_eq!(*deletions, 0, "expected 0 deletions");
            }
            _ => unreachable!(),
        }
        // Must NOT emit a ToolCall for this block.
        let tool_calls: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolCall { .. }))
            .collect();
        assert!(tool_calls.is_empty(), "Write+diff must not emit ToolCall; got {:?}", events);
    }

    // ── GAP: State resilience — tool-call header interrupts thinking block ───

    /// When a new tool-call header (`⏺ Tool: …`) appears while the parser is
    /// accumulating a `✻ Thinking` alt-marker block (no explicit close tag),
    /// the tool-call header line is non-indented and therefore closes the
    /// thinking block.  Per current behaviour, the closing line is NOT
    /// re-dispatched, so no ToolCall event is produced for it.  A Thinking
    /// event must still be emitted with the lines accumulated so far.
    #[test]
    fn tool_call_header_closes_alt_thinking_block_without_redispatch() {
        let mut p = parser();
        let mut events = p.feed("\u{2733} Thinking\n".as_bytes());
        events.extend(p.feed(b"  contemplating\n"));
        // A tool-call header is non-indented — closes the thinking block.
        events.extend(p.feed(b"\xE2\x8F\xBA Tool: Bash\n"));

        let thinking: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Thinking { .. }))
            .collect();
        assert_eq!(thinking.len(), 1, "expected one Thinking event on interrupt; got {:?}", events);
        match &thinking[0] {
            RunEvent::Thinking { text, .. } => {
                assert!(text.contains("contemplating"), "thinking body not captured: {:?}", text);
            }
            _ => unreachable!(),
        }
        // The tool-call header that triggered the close is NOT re-dispatched in
        // the alt-thinking handler, so no ToolCall event should appear here.
        let tool_calls: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolCall { .. }))
            .collect();
        assert!(
            tool_calls.is_empty(),
            "alt-thinking close does not re-dispatch the closing line; unexpected ToolCall: {:?}",
            events
        );
    }

    // ── GAP: Invalid UTF-8 bytes → System event ──────────────────────────────

    /// Bytes that are not valid UTF-8 must never panic; they must emit a
    /// `RunEvent::System` containing the lossy-decoded text.
    #[test]
    fn invalid_utf8_bytes_emit_system_event() {
        let mut p = parser();
        // \xFF and \xFE are not valid in any UTF-8 sequence.
        let events = p.feed(b"hello \xFF\xFE world\n");
        assert_eq!(events.len(), 1, "expected exactly one event; got {:?}", events);
        match &events[0] {
            RunEvent::System { text, .. } => {
                // The lossy replacement must contain the valid ASCII portions.
                assert!(text.contains("hello"), "expected 'hello' in lossy text: {:?}", text);
                assert!(text.contains("world"), "expected 'world' in lossy text: {:?}", text);
            }
            other => panic!("expected System event for invalid UTF-8, got {:?}", other),
        }
    }

    // ── GAP: Empty tool call body → ToolCall with null input ────────────────

    /// A `⏺ Tool: Foo` header followed immediately by an empty line (no
    /// indented body) must emit a `ToolCall` with `input: null`, not a
    /// `RunEvent::System` for a parse error.
    #[test]
    fn empty_tool_call_body_emits_tool_call_with_null_input() {
        let mut p = parser();
        let mut events = p.feed(b"\xE2\x8F\xBA Tool: Foo\n");
        events.extend(p.feed(b"\n")); // empty line — no body lines were ever fed

        let tool_calls: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::ToolCall { .. }))
            .collect();
        assert_eq!(tool_calls.len(), 1, "expected one ToolCall with null input; got {:?}", events);
        match &tool_calls[0] {
            RunEvent::ToolCall { name, input, .. } => {
                assert_eq!(name, "Foo");
                assert!(input.is_null(), "expected null input for empty body; got {:?}", input);
            }
            _ => unreachable!(),
        }
        // Must not emit a System event (malformed-body path must not fire).
        let system_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, RunEvent::System { .. }))
            .collect();
        assert!(system_events.is_empty(), "empty body must not produce System event; got {:?}", events);
    }

    // ── GAP: Adjacent tool calls with no blank line between ─────────────────

    /// When a second `⏺ Tool:` header arrives while the first tool-call block
    /// is still open (no blank line separator), the non-indented second header
    /// closes the first block and emits a ToolCall for Alpha (with null input).
    ///
    /// With FIX-4 applied, `dispatch_idle_line` propagates the new state returned
    /// by `process_idle_line` back through the static handler.  The Beta header
    /// opens `AccumulatingToolCall { name: "Beta", ... }` as the parser's new
    /// state.  When the subsequent empty line is fed, that state is closed and
    /// Beta is emitted as a second ToolCall with null input.
    #[test]
    fn adjacent_tool_calls_no_blank_line_between() {
        let mut p = parser();
        // First tool call — no body, no trailing blank line.
        let mut events = p.feed(b"\xE2\x8F\xBA Tool: Alpha\n");
        // Second tool call header arrives as non-indented, non-empty line —
        // closes Alpha block, emits Alpha ToolCall, and opens Beta via dispatch_idle_line.
        events.extend(p.feed(b"\xE2\x8F\xBA Tool: Beta\n"));
        // Parser is now AccumulatingToolCall { name: "Beta" }.
        // Empty line closes Beta and emits Beta ToolCall.
        events.extend(p.feed(b"\n"));

        // Both Alpha and Beta are emitted.
        let tool_calls: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let RunEvent::ToolCall { name, input, .. } = e {
                    Some((name.as_str(), input.is_null()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(tool_calls.len(), 2, "expected two ToolCall events (Alpha and Beta); got {:?}", events);
        assert_eq!(tool_calls[0], ("Alpha", true), "first call must be Alpha with null input");
        assert_eq!(tool_calls[1], ("Beta", true), "second call must be Beta with null input");
    }

    // ── GAP: CR-only (bare \r without \n) stays buffered ────────────────────

    /// A bare carriage return (`\r` without a following `\n`) is NOT a line
    /// terminator.  The byte is stored in the line buffer.  No event is emitted
    /// until a `\n` arrives.  This differs from the CRLF (`\r\n`) case where
    /// the `\r` is stripped before decoding.
    #[test]
    fn bare_cr_without_lf_does_not_trigger_line_processing() {
        let mut p = parser();
        // Feed text with a bare CR — no newline yet.
        let e1 = p.feed(b"hello\rworld");
        assert!(e1.is_empty(), "bare CR must not trigger line processing; got {:?}", e1);

        // Now supply the newline.  The \r is mid-buffer, not trailing, so it is
        // NOT stripped; the decoded line contains the embedded CR.
        let e2 = p.feed(b"\n");
        assert_eq!(e2.len(), 1, "expected one event after newline; got {:?}", e2);
        match &e2[0] {
            RunEvent::AssistantText { text, .. } => {
                // The \r is embedded in the text (not stripped, since stripping
                // only applies to a trailing \r immediately before \n).
                assert!(text.contains("hello"), "expected 'hello' in text: {:?}", text);
                assert!(text.contains("world"), "expected 'world' in text: {:?}", text);
            }
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    // ── GAP: Multi-chunk incomplete line stays buffered until newline ────────

    /// Bytes fed across multiple chunks without a trailing `\n` must all remain
    /// buffered — no event is emitted until the newline finally arrives in a
    /// later chunk.
    #[test]
    fn multi_chunk_incomplete_line_buffered_until_newline() {
        let mut p = parser();
        let e1 = p.feed(b"chunk");
        let e2 = p.feed(b"_two");
        let e3 = p.feed(b"_three");
        assert!(e1.is_empty(), "no event after chunk 1; got {:?}", e1);
        assert!(e2.is_empty(), "no event after chunk 2; got {:?}", e2);
        assert!(e3.is_empty(), "no event after chunk 3; got {:?}", e3);

        // Deliver the newline in its own chunk.
        let e4 = p.feed(b"\n");
        assert_eq!(e4.len(), 1, "expected one event once newline arrives; got {:?}", e4);
        match &e4[0] {
            RunEvent::AssistantText { text, .. } => {
                assert_eq!(text, "chunk_two_three");
            }
            other => panic!("expected AssistantText, got {:?}", other),
        }
    }

    // ── GAP: TOOL_CALL_PREFIX starts with U+23FA (⏺) ────────────────────────

    /// Guard against accidental corruption of the Unicode constant.
    /// `TOOL_CALL_PREFIX` must start with ⏺ (U+23FA, MEDIUM BLACK CIRCLE).
    #[test]
    fn tool_call_prefix_starts_with_u23fa() {
        let first_char = patterns::TOOL_CALL_PREFIX
            .chars()
            .next()
            .expect("TOOL_CALL_PREFIX must not be empty");
        assert_eq!(
            first_char, '\u{23FA}',
            "TOOL_CALL_PREFIX must start with ⏺ (U+23FA); got U+{:04X}",
            first_char as u32
        );
    }
}
