# Requirements Document: Dev Dashboard

**Project**: Dev Dashboard
**Author**: aamadeo@gmail.com
**Date**: 2026-05-18
**Status**: Final — ready for design phase

---

## 1. Goal

A local desktop application that gives a single developer a unified dashboard to manage multiple software projects on their machine. It surfaces each project's state at a glance (git status, recent runs) and acts as a launcher for predefined Claude Code sequences (multi-step automated workflows) against any project, with full visibility into each run's tool calls, file edits, and reasoning.

The app exists to remove the friction of context-switching between terminals, IDEs, and Claude CLI sessions when working across many projects.

---

## 2. Priorities (ranked)

1. **Easy to use** — primary driver. Dashboard must be legible at a glance; launching a sequence must take few clicks.
2. **Fast use** — common actions (open project, launch sequence, view run) must feel instant.
3. **Easy to automate** — sequences themselves are the automation surface; the app must make defining and re-running them frictionless.
4. **Easy to learn** — sensible defaults, discoverable controls, no required configuration to start.
5. **Performance** — important but secondary; the app is local single-user, so raw throughput is not the bottleneck.

---

## 3. User Stories

- **US-1**: As a developer, I want to see all my registered projects in one place so I don't have to remember where each lives on disk.
- **US-2**: As a developer, I want to see each project's git status (branch, dirty/clean, ahead/behind) without opening a terminal.
- **US-3**: As a developer, I want to launch a predefined Claude Code sequence against a chosen project with one click.
- **US-4**: As a developer, I want to optionally attach a markdown file as extra context before launching a sequence, so I can feed in notes or specs.
- **US-5**: As a developer, I want to watch a sequence run in real time, with tool calls, file edits, and thinking blocks rendered clearly — not as raw text.
- **US-6**: As a developer, I want to stop a running sequence at any time.
- **US-7**: As a developer, I want to run multiple sequences concurrently across different (or the same) projects.
- **US-8**: As a developer, I want to review past runs of any project, so I can see what was done and when.
- **US-9**: As a developer, I want a toast notification when a run completes or fails, so I can switch tasks while it runs.
- **US-10**: As a developer, I want the app to work the same on Windows, macOS, and Linux.
- **US-11**: As a developer, I want to choose how to handle a failed step (retry, skip, abort, or continue) so I can recover from Claude token exhaustion without losing the whole run.

---

## 4. Functional Requirements

### 4.1 Projects

- **FR-1.1**: User can register a project by selecting a local directory.
- **FR-1.2**: User can remove a project from the dashboard (does not delete files).
- **FR-1.3**: The dashboard displays each project as a card showing: name, path, current branch, git status (clean / dirty / ahead / behind), last run timestamp + outcome.
- **FR-1.4**: Git status is refreshed by polling each visible project every 10 seconds (default) and on window focus. Polling pauses when the window is hidden.
- **FR-1.5**: If a project directory no longer exists on disk, its card shows a "missing" state with options to relocate or remove.

### 4.2 Sequences

- **FR-2.1**: A sequence is a named, predefined multi-step Claude Code workflow stored as a config file in the app's data directory.
- **FR-2.2**: User can browse the list of available sequences.
- **FR-2.3**: User can launch a sequence by selecting (a) a target project and (b) a sequence.
- **FR-2.4**: Before launch, user can optionally attach a `.md` file via a file picker; its contents are passed to the sequence as additional context/prompt input.
- **FR-2.5**: There is no hard cap on concurrent runs. Multiple sequences may run simultaneously against the same or different projects.

### 4.3 Run execution

- **FR-3.1**: Each run spawns the Claude Code CLI as a child process scoped to the target project directory.
- **FR-3.2**: If the Claude CLI is not found on PATH, the app shows a setup screen with installation instructions and a path-override field instead of attempting to launch.
- **FR-3.3**: The run view streams output in real time with **parsed rendering**: tool calls, file edits (diffs), and thinking blocks are each visually distinct from plain assistant text. Raw text is not the default view.
- **FR-3.4**: The run view exposes a single **Stop** control that terminates the child process and marks the run as `stopped`. No pause/resume.
- **FR-3.5**: If the app process crashes or is force-quit while runs are active, child Claude CLI processes are killed on next launch (orphan detection via PID + run-state reconciliation at startup).
- **FR-3.6**: Run state transitions: `pending` -> `running` -> (`completed` | `failed` | `stopped`).
- **FR-3.7**: When a sequence step fails, the run pauses and presents the user with four options: Retry (re-invoke the step), Skip (advance to next step), Abort (terminate the run), Continue (re-invoke with prior partial output as context). "Continue" is the prominent default. This handles the common case of a Claude agent exhausting its token budget mid-step.

### 4.4 Run history & storage

- **FR-4.1**: Each run's transcript and metadata is persisted to `<project>/.claude/runs/<run-id>/` inside the target project itself.
- **FR-4.2**: Each run directory contains: `meta.json` (id, sequence, start/end timestamps, status, exit code, attached md path), `transcript.jsonl` (parsed events as they streamed), and `raw.log` (raw stdout/stderr for debugging).
- **FR-4.3**: User can browse past runs per project, sorted newest-first, and open any run to re-render its parsed view from the stored transcript.
- **FR-4.4**: Retention defaults: keep runs newer than 30 days **and** total run storage per project under 500 MB. Oldest runs beyond either limit are pruned on app startup and once daily. Both thresholds are user-configurable.

### 4.5 Notifications

- **FR-5.1**: When a run reaches a terminal state (`completed`, `failed`, `stopped`), an in-app toast appears with the sequence name, project name, and outcome.
- **FR-5.2**: Clicking the toast opens that run's view.
- **FR-5.3**: Toasts auto-dismiss after 8 seconds (success) or stay until dismissed (failed). Stopped runs auto-dismiss after 8 seconds (same as completed).
- **FR-5.4**: No OS-native notifications in v1.

### 4.6 Settings

- **FR-6.1**: User can configure: git poll interval, retention age, retention size cap, Claude CLI path override.
- **FR-6.2**: Settings persist in the app's per-user config directory (OS-appropriate location).

---

## 5. Non-Functional Requirements

- **NFR-1 Platform**: Tauri-based desktop app. Must build and run on Windows, macOS, and Linux from the same codebase.
- **NFR-2 Auth & access**: No authentication. No remote access. Strictly local single-user. The app never listens on a network port reachable beyond `127.0.0.1`.
- **NFR-3 Data locality**: All run data lives inside the user's own project directories. App-level data (project registry, sequences, settings) lives in the OS-standard per-user config location.
- **NFR-4 Responsiveness**: Dashboard renders within 500 ms of launch on a warm cache. Launching a sequence registers visible feedback within 200 ms.
- **NFR-5 Streaming latency**: Parsed run events appear in the run view within 250 ms of being emitted by the CLI.
- **NFR-6 Resource footprint**: Idle dashboard with 20 registered projects uses under 200 MB RAM and under 1% CPU.
- **NFR-7 Crash safety**: Run transcripts are written incrementally (append-only JSONL) so a crash mid-run leaves a recoverable, readable partial record.
- **NFR-8 No telemetry**: The app makes no outbound network calls except those initiated by the Claude CLI child process itself.

---

## 6. Out of Scope (v1)

- Multi-user support, accounts, sharing.
- Remote / browser access; mobile clients.
- Cloud sync of projects, sequences, or runs.
- Editing or authoring sequences inside the app (sequences are managed as files on disk).
- Pause/resume of runs.
- Hard concurrency limits or run queueing.
- OS-native notifications, sound, or badge counts.
- Integrations with issue trackers, CI, or chat tools.
- Diff editing / accepting individual file edits from the run view (view-only in v1).
- Per-sequence scheduling or cron-style automation.

---

## 7. Closed Questions

- **OQ-1**: Sequences remain purely local files. No shared registry.
- **OQ-2**: No "re-run with same inputs" shortcut in v1.
- **OQ-3**: Retention (age + size cap) is configurable per-project.
- **OQ-4**: Per-project run history is sufficient. No global cross-project runs view.
