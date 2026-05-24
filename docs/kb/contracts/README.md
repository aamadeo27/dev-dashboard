# Contracts

**Data Models** (Rust structs, source of truth, exported to TS via ts-rs):

- [`Project`](project.md) — registered project: id, path, tags, language, package_manager, git status
- [`Sequence`](sequence.md) — .md file in `<project>/.claude/sequences/`, loaded on demand
- [`Run`](run.md) — per-run state machine: pending→running→{completed,failed,stopped}, stored in meta.json
- [`RunEvent`](run-event.md) — 9-variant enum stored in transcript.jsonl (assistant_text, thinking, tool_call, tool_result, file_edit, user_input, system, step_failed, error)
- [`Settings`](settings.md) — app config (paths, poll intervals, retention, view mode), stored in os_config_dir
- [`UsageSnapshot`](usage-snapshot.md) — in-memory only; parsed output of `claude /usage`
- [Auth](auth.md) — no authentication; local single-user, OS filesystem permissions suffice

**IPC Commands** (all return `Result<T, AppError>`):

- [IPC: Projects](ipc-projects.md) — list, add, remove, relocate, tag, git status, open in editor/terminal
- [IPC: Sequences](ipc-sequences.md) — list_sequences, refresh_sequences
- [IPC: Runs](ipc-runs.md) — launch, stop, send_input, step_failure response, list, load_transcript
- [IPC: Settings + system](ipc-settings-system.md) — get/update settings, verify CLI, usage, logs
- [IPC: Events](ipc-events.md) — push events from Rust to frontend (run:event, git:updated, toast:show, etc.)
