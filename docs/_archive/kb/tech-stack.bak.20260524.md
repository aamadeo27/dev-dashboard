# Tech Stack

## 2. Stack Decisions

### 2.1 Tauri

- **Tauri 2.x** (latest stable as of 2026-05). v2 has stable cross-platform notification, FS, dialog, and shell plugins; sidecar/process management is well-supported.
- Why Tauri (vs Electron): smaller footprint (NFR-6: <200 MB RAM idle), native FS and process control from Rust, no bundled Chromium — matches NFR-2 (no exposed local server beyond loopback IPC).

### 2.2 Frontend: React + TypeScript + Vite

- **Framework**: **React 18 + TypeScript**, built with **Vite**.
- **Justification**:
  - Single developer + Sonnet coders: React has the largest training corpus, fewest surprise idioms.
  - Component library reuse: the UI spec has a clear component inventory (`ProjectCard`, `EventBlock`, etc.) — React's component model maps 1:1.
  - Streaming-heavy run view benefits from `useSyncExternalStore` + memoization patterns that are well-trodden in React.
  - TS is non-negotiable for the IPC contract surface (Rust types -> TS types via `ts-rs` or hand-mirrored).
- **Rejected**: Svelte (smaller ecosystem for the dev tools we need; coder unfamiliarity tax); Vue (no clear win over React here); SolidJS (too niche for a Sonnet coder to navigate confidently).

### 2.3 State management: Zustand + React Query (TanStack Query)

- **Zustand** for client-only UI state (modals open, selected project, scroll positions, toast queue, draft text in input boxes).
- **TanStack Query** for everything that originates in Rust (projects list, git status, sequences, run history, usage snapshot). Cache + invalidation handles the polling cases cleanly.
- **Live run events**: a dedicated Zustand store per active run keyed by `run_id`, populated from Tauri event subscriptions. Not in TanStack Query — these are push, not pull.
- **Rejected**: Redux Toolkit (overkill, more boilerplate than the project warrants); Context-only (re-render storms in run view).

### 2.4 IPC pattern

Two channels, both Tauri-native:

1. **Commands (request/response)**: frontend calls a typed Rust function, awaits a `Result`. Used for CRUD, launch, stop, settings, etc.
2. **Events (push)**: Rust emits to the webview using `window.emit`. Used for streaming run events, git status updates, usage refreshes, toast triggers.

Event channel naming: `<domain>:<action>`, e.g. `run:event`, `run:finished`, `git:updated`, `project:missing`, `usage:updated`, `toast:show`. Payloads are always typed JSON.

For run events, the frontend subscribes once per mounted run view to `run:event` with a payload filter `{ run_id }`. The Rust side does **not** demux per-listener — it emits all events; frontend filters. Simpler, and N is bounded by visible run views.

### 2.5 Build tooling

- **Vite** for the web side. Dev server only used in `tauri dev`.
- **Cargo** for the Rust side, workspace not needed (single crate sufficient at this size; can split later if `RunManager` grows).
- **pnpm** for JS package management (faster than npm, deterministic).
- **Biome** for JS/TS lint + format (single tool, no ESLint+Prettier config churn).
- **rustfmt + clippy** on Rust, gated in CI/local pre-commit.

### 2.6 Key Rust crates

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

### 2.7 Key JS packages

| Package | Use |
|---|---|
| `react`, `react-dom` | UI |
| `@tauri-apps/api` v2 | Command + event IPC |
| `@tauri-apps/plugin-dialog` | File pickers |
| `@tauri-apps/plugin-opener` | Open in editor/terminal (OS default app for path/URL) |
| `zustand` | Client state |
| `@tanstack/react-query` | Server-state cache |
| `react-router-dom` | Screen routing (S-01 through S-07) |
| `lucide-react` | Icons (smaller bundle, better TS types than Phosphor) |
| `react-markdown` + `remark-gfm` | Render assistant text |
| `diff2html` | Unified diff rendering for file-edit events (MIT licensed, richer rendering than `diff`) |
| `clsx` | Class composition |
