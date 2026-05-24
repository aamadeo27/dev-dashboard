# Patterns

- [Layering](layering.md) — Rust commands are thin shims; frontend components consume hooks, never invoke() directly
- [Error handling](error-handling.md) — AppError enum with typed variants; frontend translates via utils/errors.ts, never shows raw messages
- [Async and cancellation](async-and-cancellation.md) — CancellationToken per long-running task; graceful shutdown waits up to 2s
- [Streaming parser strategy](streaming-parser-strategy.md) — interactive-mode Claude CLI; heuristic line-by-line parser with known output patterns
- [Concurrency limits and backpressure](concurrency-limits-and-backpressure.md) — unbounded runs, mpsc capacity 1024, drop only assistant text on overflow
- [Step failure protocol](step-failure-protocol.md) — emit StepFailed event + step_failure signal, wait for respond_to_step_failure (60s timeout)
