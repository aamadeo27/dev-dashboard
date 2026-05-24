# Key Rust crates

| Crate | Use |
|---|---|
| `tauri` 2.x | App framework |
| `tauri-plugin-dialog` | Native file/directory pickers |
| `tauri-plugin-opener` | Open project paths in OS default editor / file manager / terminal (URLs and paths) |
| `tauri-plugin-fs` | (sparingly) FS access from frontend if needed; prefer commands |
| `tokio` | Async runtime, processes, timers |
| `git2` | Libgit2 bindings for git status |
| `serde`, `serde_json` | Serialization, JSONL |
| `chrono` | Timestamps, durations |
| `uuid` | Run IDs (v7 — time-sortable) |
| `dirs` | OS-standard config directory |
| `tracing`, `tracing-subscriber`, `tracing-appender` | Structured logging |
| `thiserror` | Error types |
| `notify` (optional) | FS watch for sequences directory (mtime fallback if too costly) |
| `sysinfo` | Orphan reaper: check if PID is alive and matches expected exe name |
| `ts-rs` | Auto-generate TS bindings from Rust structs |

**Plugin choice — `plugin-opener` vs `plugin-shell`**: we use `tauri-plugin-opener` (not `tauri-plugin-shell`) for the "Open in Editor" and "Open in Terminal" context-menu actions. `plugin-opener` is the Tauri 2 plugin specifically designed to hand a path/URL to the OS's default handler — which is exactly what these actions need. `plugin-shell` is for spawning and managing arbitrary child processes with stdin/stdout control; using it here would require a broader allowlist than necessary and exposes capabilities (arbitrary command execution from the webview) we do not want. `plugin-shell` is not used at all in v1: Claude CLI subprocesses are spawned directly via `tokio::process::Command` from the Rust core (not via the shell plugin), since they need stdin piping and stream parsing that the plugin does not provide.
