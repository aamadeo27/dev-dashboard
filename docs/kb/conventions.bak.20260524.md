# Conventions

## 4. File Layout (Codebase)

```
dev-dashboard/
├── .claude/                       # this project's own dashboard data (dogfood)
│   ├── requirements.md
│   ├── ui-ux-spec.md
│   ├── knowledge-base.md          # monolith (superseded by docs/kb/)
│   └── epics.md
├── src-tauri/                     # Rust side
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs                # thin binary entrypoint; calls `dev_dashboard_lib::run()`
│       ├── lib.rs                 # library crate root: `pub fn run()`, plugin registration, command list, module wiring
│       ├── app_state.rs           # AppState struct (held in tauri::State)
│       ├── error.rs               # AppError + AppResult
│       ├── settings.rs            # SettingsStore
│       ├── projects/
│       │   ├── mod.rs             # ProjectRegistry
│       │   ├── scanner.rs         # language/pm detection
│       │   └── git.rs             # GitPoller, git status via git2
│       ├── sequences/
│       │   └── mod.rs             # SequenceLoader, description extraction
│       ├── runs/
│       │   ├── mod.rs             # RunManager
│       │   ├── session.rs         # RunSession (per-run state)
│       │   ├── parser.rs          # EventParser (stream -> RunEvent)
│       │   ├── transcript.rs      # TranscriptWriter (JSONL + raw)
│       │   ├── orphan.rs          # OrphanReaper
│       │   └── retention.rs       # RetentionPruner
│       ├── usage.rs               # UsageProbe
│       ├── ipc/
│       │   ├── mod.rs             # command registration
│       │   ├── commands.rs        # all #[tauri::command] fns
│       │   └── events.rs          # event name constants + emit helpers
│       └── platform/
│           ├── editor.rs          # open-in-editor
│           └── terminal.rs        # open-in-terminal
├── src/                           # Frontend (React + TS)
│   ├── main.tsx
│   ├── App.tsx                    # router + global providers
│   ├── routes/
│   │   ├── Setup.tsx              # S-01
│   │   ├── Dashboard.tsx          # S-02
│   │   ├── ProjectDetail.tsx      # S-03
│   │   ├── RunLive.tsx            # S-04
│   │   ├── RunHistorical.tsx      # S-05
│   │   └── Settings.tsx           # S-07
│   ├── components/
│   │   ├── ProjectCard.tsx
│   │   ├── GitStatusBadge.tsx
│   │   ├── RunOutcomeBadge.tsx
│   │   ├── EventBlock/            # one file per event type
│   │   │   ├── AssistantBlock.tsx
│   │   │   ├── ThinkingBlock.tsx
│   │   │   ├── ToolCallBlock.tsx
│   │   │   ├── ToolResultBlock.tsx
│   │   │   ├── FileEditBlock.tsx
│   │   │   ├── UserInputBlock.tsx
│   │   │   ├── SystemBlock.tsx
│   │   │   └── StepFailedBlock.tsx
│   │   ├── LaunchModal.tsx        # S-06
│   │   ├── TagEditorPopover.tsx   # S-08
│   │   ├── Toast.tsx              # S-09
│   │   ├── RateLimitPill.tsx
│   │   └── ContextMenu.tsx
│   ├── stores/
│   │   ├── ui.ts                  # zustand: modals, view mode, etc.
│   │   ├── toasts.ts              # zustand: toast queue
│   │   └── liveRuns.ts            # zustand: live run event buffers
│   ├── ipc/
│   │   ├── commands.ts            # typed wrappers around invoke()
│   │   ├── events.ts              # typed event subscribers
│   │   └── bindings.ts            # AUTO-GENERATED from ts-rs (do not edit)
│   ├── hooks/
│   │   ├── useProjects.ts
│   │   ├── useGitStatus.ts
│   │   ├── useSequences.ts
│   │   ├── useRunHistory.ts
│   │   ├── useLiveRun.ts
│   │   ├── useUsage.ts
│   │   └── useSettings.ts
│   ├── styles/
│   │   ├── tokens.css             # CSS variables from UI spec section 1.1
│   │   └── globals.css
│   └── utils/
│       ├── format.ts              # relative timestamps, durations, sizes
│       └── markdown.ts            # safe markdown rendering
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── biome.json
└── README.md
```

**Rust crate layout — `lib.rs` + thin `main.rs`**: `src-tauri` is configured as both a library crate (`dev_dashboard_lib`) and a binary crate. `main.rs` contains only `fn main() { dev_dashboard_lib::run() }`; `lib.rs` exposes `pub fn run()` which performs plugin registration, builds `AppState`, registers the command handlers, and runs the Tauri app. This is the standard Tauri 2 pattern: it allows `cargo test` to exercise domain modules without spawning the binary, supports integration tests under `src-tauri/tests/`, and is required for mobile targets (iOS/Android) should we ever add them. The `Cargo.toml` declares both `[lib]` (name `dev_dashboard_lib`, `crate-type = ["staticlib", "cdylib", "rlib"]`) and `[[bin]]` (name `dev-dashboard`, `path = "src/main.rs"`). No business logic lives in `main.rs`.

**Domain module visibility**: Domain modules (`projects`, `runs`, `sequences`, `settings`, `usage`) are declared `pub` in `lib.rs`. This is required for integration tests under `src-tauri/tests/` (which compile as separate crates and must see `pub` items) and for the ts-rs export tooling. The crate is `cdylib`/`rlib` with no external Rust consumers; this `pub` is Tauri-internal and does not constitute a stability contract — breaking changes within these modules are allowed without deprecation.

---

## 6. Conventions (Code Style and Process)

### 6.4 File encoding and line endings

- All transcripts, logs, settings, registry: **UTF-8, no BOM, LF line endings**, regardless of platform.
- Sequence `.md` files: read as UTF-8; fall back to UTF-8 lossy if invalid bytes (log a warning).
- Paths: `PathBuf` in Rust, `string` (absolute) in TS. Never relative across the IPC boundary.

### 6.5 Naming

- Rust: `snake_case` for fns/vars, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- TS: `camelCase` for vars/fns, `PascalCase` for types and React components.
- Tauri commands: `snake_case` on the wire (Rust default). TS wrappers mirror with `camelCase`.
- Files: kebab-case for non-component files (`format.ts`, `app-state.rs`), `PascalCase.tsx` for React components.

### 6.6 Testing approach

- **Rust**: unit tests inline (`#[cfg(test)]`) for parser, retention, orphan detection, settings (de)serialization. Integration tests in `src-tauri/tests/` for filesystem-touching flows using a `tempdir`.
- **Frontend**: Vitest for utility functions and event-block components (snapshot + interaction). React Testing Library for components. Avoid testing TanStack Query plumbing — trust the lib.
- **End-to-end**: deferred until v1.1. The Tauri E2E story (WebDriver) is fragile; manual smoke checklist suffices for v1.

---

## 10. Branching and PR Pattern

Full details in `.claude/devops.md` §3 and §4. Summary for coders:

**Branch naming**: `feat/<task-id>-short-desc`, `fix/<task-id>-short-desc`, `chore/<desc>`, `docs/<desc>`, `refactor/<desc>`. Example: `feat/T0.1-scaffold`.

**Base branch**: `main` only. All branches cut from `main`, all PRs target `main`.

**Merge strategy**: squash-merge. One commit per task on `main`.

**PR title format**: `<type>(<scope>): <imperative description>` — e.g. `feat(runs): implement RunManager spawn and session lifecycle`.

**PR body must include**: What (one sentence), How (2-4 bullets of key decisions), Test (what was verified), Checklist (`lint`, `typecheck`, `clippy`, `tests`, `bindings` if Rust types changed).

**Required checks before merge** (enforced by branch protection):
- `Check (ubuntu-latest)` — Biome lint, tsc, rustfmt, clippy, vitest, cargo test, bindings freshness
- `Check (macos-latest)` — same
- `Check (windows-latest)` — same

**Commit format**: Conventional Commits — `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `perf`, `build`. Subject: imperative, max 72 chars. Body explains *why*.

**No force-push to `main`**.

---

## 11. Secrets

Full details in `.claude/devops.md` §1.

**This repo has no secrets.** The application manages zero credentials. The `claude` CLI handles its own authentication externally and independently of this codebase.

- No `.env` files, no secret manager, no credentials in CI.
- Nothing to inject at build time or runtime.
- No tokens, API keys, or passwords are stored, referenced, or passed through this app.
- Hard rules (apply even if this changes in future):
  - No secrets in git (not even gitignored `.env` with real values committed once).
  - No secrets in log output.
  - No secrets in client bundles or IPC payloads.
- Local dev story: not applicable — there is nothing to configure. Run `pnpm dev` and the app works.
