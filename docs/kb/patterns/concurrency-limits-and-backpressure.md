# Concurrency limits and backpressure

- No hard cap on concurrent runs (FR-2.5).
- Per-run event channel: `tokio::sync::mpsc` with capacity 1024. If full (slow frontend), drop **non-essential** events (assistant text only) and emit a `system` event "Event buffer overflow — UI lagged". Never drop tool calls, file edits, or terminal events.
- Transcript writes are serialized per-run (one writer task owns the file handle).
