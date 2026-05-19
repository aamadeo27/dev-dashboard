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
