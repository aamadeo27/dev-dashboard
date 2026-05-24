# Streaming parser strategy

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
