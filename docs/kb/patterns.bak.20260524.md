# Patterns

## 6. Patterns and Conventions (Architectural)

### 6.1 Layering

- **Rust**: `ipc/commands.rs` is a thin shim. All real work lives in the domain modules (`projects/`, `runs/`, etc.). Commands take `tauri::State<AppState>` and forward to domain methods. No business logic in command bodies.
- **Frontend**: route components compose feature components. Feature components consume hooks. Hooks consume `ipc/commands.ts` and `ipc/events.ts`. No direct `invoke()` calls inside components.

### 6.2 Error handling

- One `AppError` enum in Rust with concrete variants:
  - `NotFound`, `AlreadyExists`, `Io`, `Git`, `Cli`, `ParseError`, `InvalidInput`, `PermissionDenied`, `Internal`.
- Each variant serializes to `{ code: "NOT_FOUND" | ..., message, details? }`.
- The frontend has a single error-translation utility (`utils/errors.ts`) that maps codes to user-facing strings. Components never display raw `error.message` directly — always through the translator.
- Background tasks (poller, pruner, parser) **never panic**. Errors are logged via `tracing` and surfaced as `system` events or toasts where user-relevant.

### 6.3 Async and cancellation

- Every long-running task (run session, git poller, usage probe) holds a `CancellationToken` (tokio-util). Stop = cancel token, then `child.kill()` for processes.
- Window close: graceful shutdown sends cancel to all tokens, waits up to 2 seconds, then aborts.

### 6.7 Streaming parser strategy

**Decision**: launch `claude` in **interactive mode** (no `--print` flag, no `--output-format` flag). The parser is heuristic and reads the human-readable stdout that Claude emits in interactive sessions.

The Claude CLI's interactive stdout format is **the** load-bearing assumption. The parser must:
1. Buffer bytes until a newline (or `\r\n`) is seen.
2. Match each line against a set of known patterns to detect event boundaries.
3. Carry state across lines for multi-line constructs (thinking blocks, file diffs, tool inputs/outputs).
4. On no-match, emit an `AssistantText` event with the line as plain text — never drop input.
5. On internal parse error inside a recognized block, emit a `system` event with the raw line.

**Known Claude interactive output patterns to match** (subject to validation against the actual CLI build — see §9):
- Tool call header: lines beginning with `⏺ Tool: <name>` (the "⏺" sentinel marks an assistant action).
- Tool input block: indented JSON or key-value lines following a tool header until a dedent / blank line.
- Tool result: lines beginning with `⎿` (result-arrow sentinel) or `Result:` header.
- Thinking indicator: lines wrapped in `<thinking>` / `</thinking>` markers, or a leading `✻ Thinking` marker followed by indented content.
- File edit markers: a tool call to `Edit`/`Write` plus a fenced unified-diff block (`@@ ... @@` headers); the parser captures the diff body and counts additions/deletions.
- Assistant text: any line not matched by the above, after the first prompt is sent.
- Step failure: a known sentinel emitted by Claude when a step cannot continue (exact marker to be confirmed during T4.7 / §9 item 1).

**Parser implementation note**: keep pattern definitions in a single module (`runs/parser/patterns.rs`) so they can be updated as the CLI evolves. Each pattern carries a `Version` constant; the parser logs the active pattern-set version at run start for diagnostics. **These patterns must be validated against actual Claude CLI output before T4.1 is considered complete.**

### 6.8 Concurrency limits and backpressure

- No hard cap on concurrent runs (FR-2.5).
- Per-run event channel: `tokio::sync::mpsc` with capacity 1024. If full (slow frontend), drop **non-essential** events (assistant text only) and emit a `system` event "Event buffer overflow — UI lagged". Never drop tool calls, file edits, or terminal events.
- Transcript writes are serialized per-run (one writer task owns the file handle).

### 6.9 Step failure protocol

When the parser detects a step failure (`StepFailed` event):
1. Emit `run:event` with the `StepFailed` payload (UI renders inline).
2. Emit `run:step_failure` (UI surfaces the Retry/Skip/Abort/Continue prompt).
3. Wait for `respond_to_step_failure` command (with 60s default timeout -> auto-Continue).
4. Translate choice to stdin input expected by Claude CLI (exact tokens TBD by Coder during CLI integration).
