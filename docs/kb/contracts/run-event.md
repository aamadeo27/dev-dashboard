# `RunEvent`

```rust
#[serde(tag = "type", rename_all = "snake_case")]
enum RunEvent {
    AssistantText { text: String, ts: DateTime<Utc> },
    Thinking      { text: String, ts: DateTime<Utc> },
    ToolCall      { id: String, name: String, input: serde_json::Value, ts: DateTime<Utc> },
    ToolResult    { call_id: String, output: serde_json::Value, is_error: bool, ts: DateTime<Utc> },
    FileEdit      { path: String, diff: String, additions: u32, deletions: u32, ts: DateTime<Utc> },
    UserInput     { text: String, ts: DateTime<Utc> },
    System        { text: String, ts: DateTime<Utc> },
    StepFailed    { step: String, message: String, ts: DateTime<Utc> },
    Error         { message: String, ts: DateTime<Utc> },
}
```

**Storage**: `<project>/.claude/runs/<run-id>/transcript.jsonl`. One event per line. Append-only.

A second file, `raw.log`, captures unmodified stdout+stderr bytes for debugging.
