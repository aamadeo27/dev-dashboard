# Requirements Document: Dev Dashboard

**Project**: Dev Dashboard
**Author**: aamadeo@gmail.com
**Date**: 2026-05-18
**Status**: Final — ready for design phase

> [adoption-assumption] Promoted to canonical `docs/requirements.md` during project adoption (sequence 14, 2026-06-13). Body preserved verbatim from `.claude/requirements.md`; entries added below for three shipped-but-undocumented capabilities (US-12, US-13, FR-1.6, FR-7) and FR-6.1 expanded. Every original FR/NFR/US/OQ id is unchanged.

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
- **US-12**: As a developer, I want to tag my projects and filter the dashboard by tag, so I can focus on a subset (e.g. `work`, `oss`, `rust`) when I have many projects registered.
  > [adoption-assumption] Inferred from shipped `Project.tags: Vec<String>` (KB contract `project.md`), `TagEditorPopover.tsx`, and tag-chip filtering wired into `Dashboard.tsx`. Not in original `.claude/requirements.md`.
- **US-13**: As a developer, I want to see my Claude usage / rate-limit status at a glance from the dashboard, so I know whether I have budget before launching a sequence.
  > [adoption-assumption] Inferred from shipped `RateLimitPill` component + `useUsage` hook + `UsageSnapshot` contract + Epic 7 (`docs/epics/epic-7-usage/`). Not in original `.claude/requirements.md`.

---

## 4. Functional Requirements

### 4.1 Projects

- **FR-1.1**: User can register a project by selecting a local directory.
- **FR-1.2**: User can remove a project from the dashboard (does not delete files).
- **FR-1.3**: The dashboard displays each project as a card showing: name, path, current branch, git status (clean / dirty / ahead / behind), last run timestamp + outcome.
- **FR-1.4**: Git status is refreshed by polling each visible project every 10 seconds (default) and on window focus. Polling pauses when the window is hidden.
- **FR-1.5**: If a project directory no longer exists on disk, its card shows a "missing" state with options to relocate or remove.
- **FR-1.6**: Project tagging and tag-based filtering.
  > [adoption-assumption] Entire FR-1.6 added during adoption from shipped code + KB `contracts/project.md`. Verify each sub-point.
  - **FR-1.6.1**: User can add and remove free-text tags on a project via a right-click context-menu popover (`TagEditorPopover`) on the project card.
    > [adoption-assumption] Add/remove via popover confirmed in `src/components/TagEditorPopover.tsx`; right-click entry point via `ContextMenu`.
  - **FR-1.6.2**: Tags are normalized on entry: trimmed and lowercased before storage.
    > [adoption-assumption] `Project.tags` documented as "lowercased, trimmed, deduped" in `docs/kb/contracts/project.md`; `handleAdd` in `TagEditorPopover.tsx` trims + lowercases.
  - **FR-1.6.3**: Duplicate tags are silently deduplicated — adding an existing tag is a no-op.
    > [adoption-assumption] `TagEditorPopover.handleAdd` rejects when `tags.includes(trimmed)`.
  - **FR-1.6.4**: Each tag is capped at **32 characters**. Enforced in **both** the UI (tag input `maxLength=32`) and the backend (`set_project_tags` rejects/truncates over-length tags). [Resolved OQ-5, adoption 2026-06-13.]
    > [adoption-assumption] Limit (32) and dual enforcement are a confirmed product decision. Enforcement was NOT present in code at adoption time — tracked by task T2.9 (see `docs/epics/epic-2-project-registry-git/T2.9.md`).
  - **FR-1.6.5**: The dashboard renders the union of all tags as filter chips; selecting chips filters the visible projects. Multiple selected chips combine with **AND** semantics (a project must carry every selected tag to remain visible).
    > Code-confirmed AND: `selectedTags.every(...)` in `src/routes/Dashboard.tsx`. [Resolved OQ-6, adoption 2026-06-13.]

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

- **FR-6.1**: User can configure the following persisted settings:
  > [adoption-assumption] Original FR-6.1 listed 4 settings (git poll interval, retention age, retention size cap, Claude CLI path override). Expanded to the 7 fields actually persisted, per KB contract `docs/kb/contracts/settings.md` (`struct Settings`). Verify defaults/ranges below.
  - **git poll interval** — `git_poll_interval_secs`, default 10, min 5, max 3600. *(original)*
  - **retention age** — `retention_days`, default 30, min 1. *(original)*
  - **retention size cap** — `retention_size_mb`, default 500, min 50. *(original)*
  - **Claude CLI path override** — `claude_cli_path`, optional; overrides PATH lookup. *(original)*
  - **projects parent directory** — `parent_dir`, optional; single configured parent dir for project discovery (KB GAP-07).
    > [adoption-assumption] Persisted as `Settings.parent_dir: Option<PathBuf>`; not in original FR-6.1.
  - **dashboard view mode** — `view_mode`, enum `Grid | List`.
    > [adoption-assumption] Persisted as `Settings.view_mode: ViewMode`; not in original FR-6.1.
  - **usage poll interval** — `usage_poll_interval_secs`, range 30–3600s.
    > Persisted as `Settings.usage_poll_interval_secs: u32`. Default **60s**, range 30–3600s (KB `contracts/settings.md`). [Resolved OQ-7, adoption 2026-06-13.] Drives FR-7 polling cadence.
- **FR-6.2**: Settings persist in the app's per-user config directory (OS-appropriate location).
  > [adoption-assumption] Confirmed storage path `<os_config_dir>/dev-dashboard/settings.json` (KB `contracts/settings.md`).

### 4.7 Usage Monitoring

> [adoption-assumption] Entire FR-7 section added during adoption from shipped `UsageSnapshot` contract, `RateLimitPill`/`useUsage` frontend, and Epic 7 (`docs/epics/epic-7-usage/`). The pill UI was still a stub on branch `feat/T4.7-step-failure-command` at adoption time; backend contract + epic spec are the evidence. Verify behavior once Epic 7 lands.

- **FR-7.1**: The dashboard top bar shows a **usage pill** summarizing current Claude usage / rate-limit status (US-13).
  > [adoption-assumption] `RateLimitPill` component + Dashboard top-bar placement (Epic 7 / UI spec §5.2).
- **FR-7.2**: Usage is obtained by running the Claude CLI subcommand `claude /usage` as a short-lived local subprocess (10s timeout) and parsing its stdout into an ordered key-value map.
  > [adoption-assumption] `UsageSnapshot { parsed: BTreeMap<String,String>, raw_stdout, fetched_at, available }` (KB `contracts/usage-snapshot.md`); Epic 7 T7.1 describes `<claude> /usage`, 10s timeout, KV parse.
- **FR-7.3**: Usage is re-probed on a timer (interval = `usage_poll_interval_secs`, see FR-6.1) and on window focus; polling pauses on window blur.
  > [adoption-assumption] Epic 7 T7.1 ("every 60s + on-demand, pauses on window blur"); cadence bound to the persisted `usage_poll_interval_secs` setting.
- **FR-7.4**: If the subprocess fails (CLI missing, non-zero exit, timeout, unparseable output), `UsageSnapshot.available` is `false`, the failure is logged, no exception propagates, and the pill renders a `--` placeholder.
  > [adoption-assumption] Epic 7 T7.1/T7.2 acceptance criteria; `available: bool` flag in contract.
- **FR-7.5**: Clicking the pill opens a popover showing the full parsed key-value usage output plus a **Refresh** control that triggers an immediate re-probe (with a spinner while in flight).
  > [adoption-assumption] Epic 7 T7.2 (popover with full KV list + Refresh calling `refresh_usage`, spinner during refresh).
- **FR-7.6**: The usage probe is a **local subprocess only** — it makes no direct outbound network calls from the app and emits no telemetry. Any network traffic is performed by the Claude CLI child process itself, consistent with **NFR-8**.
  > [adoption-assumption] Cross-reference to NFR-8; the app shells out to the local `claude` binary rather than calling any usage API directly.

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

---

## 8. Open Questions (adoption)

> [adoption-assumption] These are the only fields the user must verify; everything else is preserved verbatim or grounded in code/KB contracts.

- **OQ-5**: ✓ RESOLVED (2026-06-13) — per-tag limit is **32 chars, enforced UI + backend** (FR-1.6.4). Implementation tracked by T2.9.
- **OQ-6**: ✓ RESOLVED — tag filtering uses **AND** semantics; code-confirmed (`selectedTags.every(...)` in `src/routes/Dashboard.tsx`).
- **OQ-7**: ✓ RESOLVED — `usage_poll_interval_secs` default **60s**, range 30–3600s (KB `contracts/settings.md`).
