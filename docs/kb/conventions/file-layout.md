# File Layout (Codebase)

```
dev-dashboard/
├── docs/                          # canonical spec + knowledge base
│   ├── requirements.md            # was .claude/requirements.md (relocated, adoption 2026-06-13)
│   ├── ui-ux.md                   # was .claude/ui-ux-spec.md
│   ├── devops.md                  # was .claude/devops.md
│   ├── monitoring.md              # was .claude/monitoring.md
│   ├── kb/                        # itemized knowledge base (index-only README)
│   ├── epics/                     # epic-0..9 + README backlog
│   └── _archive/                  # superseded originals + pre-split .bak files
├── .claude/                       # agent/sequence/hook config (spec docs relocated to docs/)
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
