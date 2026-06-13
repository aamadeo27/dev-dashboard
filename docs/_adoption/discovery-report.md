# Discovery Report — Dev Dashboard (project adoption)

**Generated**: 2026-06-13 by orchestrator (sequence 14-project-adoption, mode=autonomous)
**Purpose**: single source of truth for all adoption agents. Do NOT re-scan the repo — read this.

## Stack
- **Tauri 2** desktop app, cross-platform (Windows/macOS/Linux), single-user local.
- **Backend**: Rust (`src-tauri/`). Edition per `Cargo.toml`. ts-rs for IPC binding export.
- **Frontend**: React 19 + TypeScript ~5.8 + Vite 7, `src/`.
- **Pkg mgr**: pnpm. **Test**: vitest (FE) + cargo test (BE). **Lint**: biome (FE) + clippy/rustfmt (BE).
- **Deps**: @tanstack/react-query, react-router-dom 7, zustand 5, react-markdown+remark-gfm, lucide-react, clsx, @tauri-apps/{api,plugin-dialog,plugin-opener}, diff2html.

## Entry points
- Frontend: `src/main.tsx` → `src/App.tsx` (react-router routes).
- Backend: `src-tauri/src/main.rs` → `lib.rs`.

## Backend module map (`src-tauri/src/`)
- `app_state.rs`, `error.rs`, `logging.rs`
- `ipc/` — `commands.rs`, `events.rs`, `cli_watcher.rs`, `mod.rs`
- `platform/` — `editor.rs`, `terminal.rs`, `mod.rs`
- `projects/` — `git.rs`, `scanner.rs`, `mod.rs`
- `runs/` — `manager.rs`, `parser.rs` (+ `parser/patterns.rs`), `orphan.rs`, `mod.rs`
- `capabilities/default.json`, `tauri.conf.json`, `build.rs`, `tests/`

## Frontend map (`src/`)
- Routes: `Dashboard`, `ProjectDetail`, `RunLive`, `RunHistorical`, `Settings`, `Setup`.
- Components: `ProjectCard`, `GitStatusBadge`, `RunOutcomeBadge`, `SequenceList`/`SequenceRow`, `LaunchModal`, `ContextMenu`, `TagEditorPopover`, `RateLimitPill`, `Toast`, `EventBlock/*` (Assistant/FileEdit/StepFailed/System/Thinking/ToolCall/ToolResult/UserInput), `common/ErrorBoundary`.
- Hooks: `useProjects`, `useGitStatus`, `useSequences`, `useSettings`, `useLiveRun`, `useRunHistory`, `useUsage`.
- Stores (zustand): `liveRuns`, `toasts`, `ui`.
- IPC: `ipc/bindings.ts` (generated), `commands.ts`, `events.ts`.
- Utils: `env`, `errors`, `format`, `markdown`, `placeholder`. Styles: `tokens.css`, `globals.css`.

## Data / storage (no DB)
- App data (registry, sequences, settings): OS per-user config dir (`~/.config/dev-dashboard/`, `%APPDATA%\dev-dashboard\`, `~/Library/Application Support/dev-dashboard/`).
- Run data: `<project>/.claude/runs/<run-id>/` — `meta.json`, `transcript.jsonl`, `raw.log`.
- Logs: `<os_config_dir>/dev-dashboard/logs/` (daily-rotated JSON via tracing).

## CI / infra
- `.github/workflows/ci.yml` (lint/typecheck/clippy/fmt/test/bindings on 3 OS), `build.yml` (tag-triggered cross-platform bundles).
- `.githooks/pre-commit` (lint:ci + typecheck). `.claude/hooks/` — `update-status.sh`, `validate-task-footer.sh`, `log-usage.sh`.
- `.github/dependabot.yml` present.
- **No secrets** — claude CLI auths externally; no `.env`.

## Auth / observability
- Auth: none (NFR-2, local only, 127.0.0.1 max).
- Observability: local structured `tracing` logs + in-app health pills. No telemetry (NFR-8). Full schema in `.claude/monitoring.md`.

## Existing documentation (pre-adoption locations)
- `.claude/requirements.md` — **final** requirements (11 user stories, FR-1..6, NFR-1..8, out-of-scope, closed questions). Canonical-quality.
- `.claude/ui-ux-spec.md` — **final** UI/UX spec (design system, 9 screens S-01..S-09, nav/flow maps, component lib, edge cases, all gaps resolved).
- `.claude/devops.md` — full DevOps plan (workflow, branching, commits, CI/build, releases, deps, infra).
- `.claude/monitoring.md` — full monitoring config (log schema, instrumentation points, health signals, Epic 9 tasks).
- `docs/kb/` — **already canonical**: index-only README + `system-design.md`, `tech-stack/`, `patterns/`, `contracts/`, `conventions/` (all itemized), `common-pitfalls.md`.
- `docs/epics/` — 10 epics (epic-0..9), README with dependency graph + coverage map. Epic 2 & 3 marked ✓ done; Epic 4 in progress (current branch feat/T4.7-step-failure-command, tasks merged through T4.6).
- Stray: `docs/T4.3-sec-fixes-2.md` (review artifact), `docs/*.bak.20260524.md` + `docs/kb/*.bak.20260524.md` (pre-split/itemize backups).

## Canonical gaps (what adoption must produce)
1. `docs/requirements.md` (currently `.claude/requirements.md`).
2. `docs/ui-ux.md` (currently `.claude/ui-ux-spec.md`).
3. `docs/monitoring.md` (currently `.claude/monitoring.md`).
4. `docs/devops.md` (currently `.claude/devops.md`).
5. `PROJECT_STATUS.md` — absent. No `.claude/templates/` dir exists → synthesize from KB+epics.
6. Reference fixups: `requirements.md`/`ui-ux-spec.md` bare refs in `docs/epics/README.md` and `docs/kb/conventions/file-layout.md`.
7. Archive `docs/**/*.bak.*`.

## Notes for agents
- Requirements & UI/UX are FINAL — pre-populate from them verbatim; interview the user ONLY on genuine contradictions vs observed code. Do not re-derive from scratch.
- KB & epics already conform to canonical layout — validate, do not rebuild.
- tech-decision-mode = autonomous.
