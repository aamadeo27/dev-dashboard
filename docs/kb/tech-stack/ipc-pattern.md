# IPC pattern

Two channels, both Tauri-native:

1. **Commands (request/response)**: frontend calls a typed Rust function, awaits a `Result`. Used for CRUD, launch, stop, settings, etc.
2. **Events (push)**: Rust emits to the webview using `window.emit`. Used for streaming run events, git status updates, usage refreshes, toast triggers.

Event channel naming: `<domain>:<action>`, e.g. `run:event`, `run:finished`, `git:updated`, `project:missing`, `usage:updated`, `toast:show`. Payloads are always typed JSON.

For run events, the frontend subscribes once per mounted run view to `run:event` with a payload filter `{ run_id }`. The Rust side does **not** demux per-listener — it emits all events; frontend filters. Simpler, and N is bounded by visible run views.
