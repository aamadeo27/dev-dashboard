/// Pattern version tag — bump when heuristics change so callers can detect schema drift.
pub const PATTERN_VERSION: &str = "v1-heuristic-2026-05-23";

// TODO(KB§9-item1): validate all patterns below against live Claude CLI output
// before T4.8 ships; no baseline has been established yet.

/// Sentinel line that triggers a `StepFailed` event.
///
/// Set to `"STEP_FAILED"` as the pinned mock marker for T4.7.
/// A mock CLI can emit this exact line to exercise the step-failure protocol.
/// Will be replaced with the real Claude CLI marker once live behaviour is
/// captured (KB §9 item 1).
pub const STEP_FAILED_SENTINEL: &str = "STEP_FAILED";

/// Unicode prefix that marks the beginning of a tool call line.
/// Example: `⏺ Tool: Read`
pub const TOOL_CALL_PREFIX: &str = "\u{23FA} Tool: "; // ⏺

/// Unicode prefix (or line start) that marks a tool result.
/// Example: `⎿  output text`
pub const TOOL_RESULT_PREFIX: &str = "\u{2380}"; // ⎿

/// Alternate keyword that also opens a tool-result block.
pub const TOOL_RESULT_KEYWORD: &str = "Result:";

/// Opening XML tag for a thinking block.
pub const THINKING_OPEN: &str = "<thinking>";

/// Closing XML tag for a thinking block.
pub const THINKING_CLOSE: &str = "</thinking>";

/// Alternative marker line that opens a thinking block (no closing tag — ends on
/// first empty line or non-indented line after it).
pub const THINKING_ALT_MARKER: &str = "\u{2733} Thinking"; // ✻ Thinking

/// Tool names that turn a tool-call block into a diff/FileEdit block.
pub const EDIT_TOOL_NAMES: &[&str] = &["Edit", "Write"];

/// Diff hunk header prefix — lines starting with this belong to a file diff.
pub const DIFF_HUNK_PREFIX: &str = "@@";
