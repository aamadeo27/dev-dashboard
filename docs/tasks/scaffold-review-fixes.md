# Scaffold Review Fixes — CRITICAL / HIGH / MEDIUM

**Task**: Fix all CRITICAL and HIGH findings (plus quick-win MEDIUM fixes) from the scaffold review.

---

## What was done

Applied 13 discrete fixes across security, Rust module layout, frontend routing, CI hardening, and tooling configuration. No new features; all changes are corrective or structural.

---

## How the affected components work

### Tauri CSP (C1)
The `security.csp` field in `tauri.conf.json` is passed directly to the WebView at app boot. It gates what resources the embedded page can load or contact. The chosen policy allows only `'self'` for scripts, permits `unsafe-inline` for CSS (required by React-in-Tauri without a nonce setup), and whitelists the Tauri IPC endpoints (`ipc:`, `http://ipc.localhost`).

### Rust module layout (C2)
`lib.rs` is the crate root. Every domain module must be declared here with a `mod` statement before Cargo can compile it. Stub files contain a single comment line so `rustc` accepts them as valid Rust source. `pub(crate) mod` restricts visibility to within the crate; `pub mod ipc` stays `pub` because Tauri's `generate_handler!` macro needs to reach command symbols from an external context during macro expansion.

The `errors.rs` → `error.rs` and `state.rs` → `app_state.rs` renames bring file names into alignment with KB §4 and KB §6.5 (kebab/snake-case for non-component files; the KB lists `error.rs` and `app_state.rs` explicitly).

### Logging guard (H1)
`tracing_appender::non_blocking` returns a `(NonBlocking, WorkerGuard)` pair. The guard must be held for the app lifetime — dropping it silently stops log flushing. Naming it `_guard` signals "intentionally unused" to rustc but is misleading (the guard IS used — it is returned). Renaming to `guard` removes that signal and makes the return value clearly intentional.

### Tokio features (H2)
`features = ["full"]` compiles all Tokio subsystems including `io-util`, `io-std`, `net`, `signal`, `time`, `test-util`, etc. The app only needs the async runtime, sync primitives, process spawning, and file I/O. Narrowing reduces compile time and binary size.

### HashRouter (H3 / H6)
Tauri serves the frontend from a `asset://` or `tauri://` URL scheme, not from a server with a real path hierarchy. `BrowserRouter` relies on the History API and real URL paths, which don't work under a file-like scheme. `HashRouter` stores the route in `window.location.hash`, which works correctly in any origin.

### GitHub Actions SHA pinning (H4)
Pinning to commit SHAs prevents supply-chain attacks where a tag (e.g. `@v4`) is moved to a malicious commit. The version comment (e.g. `# v4.2.2`) preserves human readability. `permissions: contents: read` follows least-privilege; `concurrency` cancels stale CI runs on force-push to save runner minutes.

### Cargo test step (H5)
Adds `cargo test --manifest-path src-tauri/Cargo.toml` after Clippy. This runs all `#[cfg(test)]` inline tests and any `tests/` integration tests, gating CI on Rust correctness not just linting.

### Route stubs + lazy loading (H6)
Route components are split into `src/routes/` to match KB §4. `React.lazy` + `Suspense` defers their JS chunk loading until first navigation, reducing initial bundle parse time. `fallback={null}` is intentional — there is no loading screen in v1.

### TanStack Query provider (H7)
`QueryClientProvider` must wrap the entire app so any hook using `useQuery` / `useMutation` can access the shared cache. A single `QueryClient` instance is created outside `render()` so it survives React re-renders. This is required before any data-fetching hooks (T-prefixed tasks) can be implemented.

### Biome ignore narrowing (H8)
`src/ipc/` contains hand-written files (`commands.ts`, `events.ts`) that should be linted. Only `src/ipc/bindings/` holds auto-generated output that must not be linted. Broadening the ignore to the whole `src/ipc/` folder would hide lint errors in the hand-written wrappers.

### Capabilities narrowing (M1)
`core:default` is a bundle that includes every core permission. Listing specific sub-permissions (`core:event:default`, etc.) follows least-privilege and makes the capability surface explicit and reviewable.

### ErrorBoundary raw error removal (M2)
Rendering `error.message` in the DOM can leak internal stack details or path information in production. The replacement shows a static string and directs the user to logs, where the full error is captured via `console.error`.

### Vite host binding (M3)
`host: "127.0.0.1"` prevents the Vite dev server from binding to `0.0.0.0` (all interfaces), which would expose the dev hot-reload server on the LAN during development. Tauri's `devUrl` already points to `localhost:1420`.

### Template cleanup (M4)
`App.css` and `react.svg` are Vite template leftovers with no consumers after the router rewrite. Leaving them causes Biome to check unused files and wastes bundle analysis surface.

---

## Files touched

| File | Change |
|---|---|
| `src-tauri/tauri.conf.json` | C1: set CSP policy |
| `src-tauri/src/lib.rs` | C2: rename modules, add `platform`, change to `pub(crate)` |
| `src-tauri/src/projects/mod.rs` | C2: declare `scanner` and `git` submodules |
| `src-tauri/src/projects/scanner.rs` | C2: new stub |
| `src-tauri/src/projects/git.rs` | C2: new stub |
| `src-tauri/src/runs/mod.rs` | C2: declare `session`, `parser`, `transcript`, `orphan`, `retention` |
| `src-tauri/src/runs/session.rs` | C2: new stub |
| `src-tauri/src/runs/parser.rs` | C2: new stub |
| `src-tauri/src/runs/transcript.rs` | C2: new stub |
| `src-tauri/src/runs/orphan.rs` | C2: new stub |
| `src-tauri/src/runs/retention.rs` | C2: new stub |
| `src-tauri/src/platform/mod.rs` | C2: new stub |
| `src-tauri/src/platform/editor.rs` | C2: new stub |
| `src-tauri/src/platform/terminal.rs` | C2: new stub |
| `src-tauri/src/error.rs` | C2: renamed from `errors.rs` |
| `src-tauri/src/app_state.rs` | C2: renamed from `state.rs` |
| `src-tauri/src/logging.rs` | H1: rename `_guard` to `guard` |
| `src-tauri/Cargo.toml` | H2: narrow tokio features |
| `src/App.tsx` | H3+H6: HashRouter, lazy imports, Suspense, route components |
| `.github/workflows/ci.yml` | H4+H5: SHA pins, permissions, concurrency, cargo test step |
| `.github/workflows/build.yml` | H4: SHA pins |
| `src/routes/Setup.tsx` | H6: new stub |
| `src/routes/Dashboard.tsx` | H6: new stub |
| `src/routes/ProjectDetail.tsx` | H6: new stub |
| `src/routes/RunLive.tsx` | H6: new stub |
| `src/routes/RunHistorical.tsx` | H6: new stub |
| `src/routes/Settings.tsx` | H6: new stub |
| `src/main.tsx` | H7: QueryClientProvider |
| `biome.json` | H8: narrow ignore to `src/ipc/bindings/` |
| `src-tauri/capabilities/default.json` | M1: replace `core:default` with specific permissions |
| `src/components/common/ErrorBoundary.tsx` | M2: remove raw error from DOM |
| `vite.config.ts` | M3: pin dev server to 127.0.0.1 |
| `src/App.css` | M4: deleted |
| `src/assets/react.svg` | M4: deleted |

---

## Decisions made within task scope

- `pub mod ipc` stays `pub` (not `pub(crate)`) because Tauri's `generate_handler!` macro expands in `lib.rs` and needs to resolve command symbols; making it `pub(crate)` is fine for internal use but `pub` is conservative and matches the Tauri scaffold convention.
- `Suspense fallback={null}` chosen per spec — no loading indicator in v1.
- Biome auto-fix (`pnpm lint`) was run to resolve import ordering differences in pre-existing files; no logic changes were made by the formatter.

---

## How to test / verify

```sh
pnpm install       # must complete without errors
pnpm lint:ci       # must show "No fixes applied"
pnpm typecheck     # must produce no output (exit 0)
pnpm test          # must show 4 passed
```

To verify Rust module declarations compile, run (requires Rust toolchain and system Tauri deps):
```sh
cargo check --manifest-path src-tauri/Cargo.toml
```
