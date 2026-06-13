# Epic 0 — Project Bootstrap and Plumbing

Foundation. Blocks everything.

---

### T0.1 [shared] Scaffold Tauri 2 + React + TS project

- **Description**: `pnpm create tauri-app` with React + TS + Vite template, Tauri v2. Rename to `dev-dashboard`. Configure Biome, rustfmt, clippy. Add the file layout from KB §4 as empty modules/files (no logic yet).
- **Acceptance**:
  - `pnpm tauri dev` launches an empty window on the dev machine.
  - `pnpm lint` and `cargo clippy -- -D warnings` both pass on the scaffold.
  - Directory tree matches KB §4 (empty files/`pub mod` declarations are fine).
- **Dependencies**: none.

---

### T0.2 [backend] AppState, AppError, command registration skeleton

- **Description**: Implement `AppState` (held in `tauri::State`), `AppError` enum + `AppResult<T>`, error serialization to `{ code, message, details? }`. Register a single `ping() -> String` command end-to-end as a smoke test.
- **Acceptance**:
  - `AppError` variants per KB §6.2; `From<std::io::Error>` and `From<git2::Error>` impls.
  - Frontend can `invoke('ping')` and get back `"pong"`.
  - Throwing an `AppError::NotFound` in a command results in a structured JS error object on the frontend.
  - If `DEV_DASHBOARD_CONFIG_DIR` env var is set, use it as the config directory instead of the OS default. Log the resolved path at INFO level on startup.
- **Dependencies**: T0.1.

---

### T0.3 [shared] ts-rs binding generation

- **Description**: Wire `ts-rs` so all DTOs in KB §3 export to `src/ipc/bindings.ts`. Add a `pnpm bindings` script that runs `cargo test --features export-bindings`. Add a CI/pre-commit check that the generated file is up to date.
- **Acceptance**:
  - Adding a `#[derive(TS)]` to a struct regenerates the TS file.
  - `Project`, `Sequence`, `Run`, `RunEvent`, `Settings`, `UsageSnapshot`, `GitStatus`, `LaunchInput`, `StepFailureChoice`, `CliCheck` all exported.
- **Dependencies**: T0.2.

---

### T0.4 [shared] IPC wrappers and event bus

- **Description**: Build `src/ipc/commands.ts` with typed wrappers for every command in KB §5 (stubbed against not-yet-implemented Rust commands is fine — types only). Build `src/ipc/events.ts` with a typed `subscribe(eventName, handler)` helper. Centralize event-name constants in `src/ipc/events.ts` and `src-tauri/src/ipc/events.rs`.
- **Acceptance**:
  - No string event names appear outside `events.ts` / `events.rs`.
  - All command wrappers compile against `bindings.ts` types.
- **Dependencies**: T0.3.

---

### T0.5 [frontend] Design tokens, global styles, router shell

- **Description**: Translate UI spec §1 into `src/styles/tokens.css` (all CSS variables). Set up `react-router-dom` with placeholder routes for S-01 through S-07. App shell renders the appropriate empty placeholder for each route.
- **Acceptance**:
  - Navigating between routes works; placeholders show the screen ID.
  - Token vars are accessible from any component.
  - Reduced-motion media query disables transitions globally (UI §1.6).
- **Dependencies**: T0.1.

---

### T0.6 [backend] Logging and tracing setup

- **Description**: Initialize `tracing` + `tracing-subscriber` with JSON formatter and daily-rotating file appender at `<os_config_dir>/dev-dashboard/logs/`. Read log level from `DEV_DASHBOARD_LOG` env var. Add a `log:frontend_error` command that ingests frontend errors.
- **Acceptance**:
  - Log file appears in the OS config dir on launch.
  - `tracing::info!("hello")` from main shows up structured in the log.
  - Calling `invoke('log_frontend_error', { message, stack })` writes a line at `error` level with `source=frontend`.
- **Dependencies**: T0.2.

---

## Infra Gap Tasks (added 2026-06-13, adoption audit)

The following tasks were identified during DevOps adoption audit. They address gaps between the DevOps plan
(`docs/devops.md`) and the actual on-disk state. None of these block feature work; they are housekeeping.

---

### T0.CI-1 [shared] Add CI workflow to enforce task-footer validation on PRs

- **Description**: `.claude/hooks/validate-task-footer.sh` exists and validates that squash-merge commits
  targeting `main` carry `Task: T<n>.<m>` and `Notes:` footers. The hook can run in CI mode (reads from
  `HEAD` when no `$1` arg is given). However, no GitHub Actions workflow invokes it on PRs. Create
  `.github/workflows/validate-task-footer.yml` that runs the script on pull requests targeting `main`,
  checking the PR's head commit message for the required footers. The job should only fail when
  `GITHUB_BASE_REF=main` (the script already checks `target_branch`).
- **Acceptance**:
  - A PR to `main` whose latest commit lacks `Task: T<n>.<m>` causes the new CI job to fail.
  - A PR to `main` with both footers passes.
  - The workflow does not run on pushes to non-`main` branches or on tag pushes.
  - The new job name is added to branch-protection required checks in the GitHub repo settings.
- **Dependencies**: none (`.claude/hooks/validate-task-footer.sh` already exists).

---

### T0.CI-2 [shared] Wire update-status.sh as a git post-merge hook via pnpm prepare ✓ DONE

> Resolved by orchestrator during adoption (2026-06-13) via option (b): `.githooks/post-merge` shim delegates to `.claude/hooks/update-status.sh` (with `INTEGRATION_BRANCH=master`), and `core.hooksPath` is set to `.githooks`. `pnpm prepare` keeps it active after clone. Note: this repo's integration branch is `master`, not `main` as the original description assumed.

- **Description**: `.claude/hooks/update-status.sh` is a `post-merge` hook that triggers the
  `status-updater` agent after a squash-merge to `main`. Currently `pnpm prepare` only sets
  `core.hooksPath .githooks`, and `update-status.sh` lives in `.claude/hooks/` — it is never invoked
  by git. Two options: (a) copy/symlink `update-status.sh` into `.githooks/post-merge`, or (b) add a
  `.githooks/post-merge` shim that delegates to `.claude/hooks/update-status.sh`. Either way,
  `pnpm prepare` must result in the hook being active after clone.
- **Acceptance**:
  - After `pnpm prepare`, merging a commit with a `Task:` footer to `main` triggers the status-updater
    agent (or logs a skip if `PROJECT_STATUS.md` does not exist yet).
  - The hook exits 0 regardless of agent outcome (best-effort, must not block merge).
  - `pnpm prepare` is idempotent: running it twice does not break anything.
- **Dependencies**: none (`update-status.sh` already exists and is correct).

---

### T0.CI-3 [shared] Wire log-usage.sh in .claude/settings.json

- **Description**: `.claude/hooks/log-usage.sh` logs Claude token usage to `DevTeam.log` on every
  Claude Code session stop. It requires a `Stop` and `SubagentStop` hook entry in `.claude/settings.json`.
  Currently only `.claude/settings.local.json` exists (permissions allowlist only); there is no
  `.claude/settings.json`. Create `.claude/settings.json` with the two hook entries pointing to
  `.claude/hooks/log-usage.sh`. The file should be committed so the hook is active for all contributors.
- **Acceptance**:
  - `.claude/settings.json` contains `hooks.Stop` and `hooks.SubagentStop` entries referencing `log-usage.sh`.
  - After a Claude Code session, a usage line appears in `DevTeam.log`:
    `[<ts>] [usage-hook] [Usage tokens] in=N out=N cache_read=N cache_create=N model=<m> source=stop`.
  - `jq` is listed as a dev prerequisite in README or CLAUDE.md (the script requires it).
- **Dependencies**: none (`log-usage.sh` already exists and is correct).

---

### T0.CI-4 [docs] Update KB branching-and-pr-pattern.md to reference docs/devops.md ✓ DONE

> Resolved by orchestrator during adoption (2026-06-13): `branching-and-pr-pattern.md` and `secrets.md` now reference `docs/devops.md`. `grep -r '\.claude/devops\.md' docs/` returns no matches.

- **Description**: `docs/kb/conventions/branching-and-pr-pattern.md` currently reads "Full details in
  `.claude/devops.md` §3 and §4." The DevOps plan has been relocated to `docs/devops.md` as part of
  the adoption process. Update the reference in that file (and any other KB/epic files that point to
  `.claude/devops.md`) to reference `docs/devops.md` instead.
- **Acceptance**:
  - `grep -r '\.claude/devops\.md' docs/` returns no matches.
  - `docs/kb/conventions/branching-and-pr-pattern.md` links to `docs/devops.md §3` and `§4`.
  - No other docs reference the old `.claude/devops.md` path.
- **Dependencies**: none (docs/devops.md already exists).
