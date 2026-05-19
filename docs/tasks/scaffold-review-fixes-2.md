# Scaffold Review Fixes — Round 2 (Perf / Security / Scope)

**Task**: Apply remaining review findings across security, performance, and scope items left over from the first review pass.

---

## What was done

Applied 12 discrete fixes covering dependency hygiene, CSP hardening, capability narrowing, CI permissions, Rust build config, CSS selector correctness, Vite optimization, Biome ignore path correction, CSS file rename, Rust plugin registration, and frontend stub file creation.

---

## How the affected components work

### diff2html in devDependencies (Item 1)
`diff2html` renders unified diffs in the `DiffBlock` component (T5.x, not yet built). Moving it to `devDependencies` removes it from the production bundle until it is actually imported. When T5.x lands, the dev will move it back to `dependencies` and add a dynamic `import()` at the call site.

### CSP style-src (Item 2)
`unsafe-inline` in `style-src` allows arbitrary inline `<style>` tags and `style=` attributes, which is exploitable via XSS. React does not require it — React applies styles via CSS classes and the `style` prop, which Tauri's WebView handles without needing the unsafe directive. Removing it closes the vector with no functional impact.

### Capability narrowing (Item 3)
`opener:default` is a wildcard that grants all opener plugin permissions. The app only needs two: opening a URL in the default browser and opening a path in the OS file manager/editor. Listing them explicitly makes the permission surface reviewable and follows least-privilege.

### Workflow permissions: contents: read (Item 4)
GitHub Actions workflows inherit `contents: write` by default in some contexts, which is broader than the build job needs (it only checks out code). Adding `permissions: contents: read` at the workflow level applies least-privilege to all jobs. The `release` job overrides with `contents: write` at job scope, which is correct — that job creates the GitHub Release.

### Cargo crate-type (Item 5)
`staticlib` is needed only for iOS/Android mobile targets where Tauri links the Rust code as a static library. For desktop-only builds it causes the linker to produce an extra `.a` artifact on every build, adding compile time and disk usage. The KB §4 note mentions mobile as a future possibility, not current scope, so `["cdylib", "rlib"]` is the correct set: `cdylib` for the Tauri-loaded shared library, `rlib` for `cargo test`.

### Firefox scrollbar selector (Item 6)
The Firefox scrollbar properties (`scrollbar-width`, `scrollbar-color`) on the `*` selector apply to every element in the DOM. This causes redundant style recalculations on large component trees. These properties cascade from `html` just as well — setting them once on `html` achieves the same visual result with no per-element overhead.

### Vite optimizeDeps (Item 7)
`lucide-react` exports hundreds of SVG components. Without explicit pre-bundling, Vite discovers and processes each one lazily during dev server warm-up, causing a cascade of HTTP requests on first load. Adding it to `optimizeDeps.include` tells Vite to pre-bundle it eagerly at dev server start, eliminating the warm-up lag.

### Biome ignore path (Item 8)
The previous ignore entry `src/ipc/bindings/` treated `bindings` as a directory. KB §4 specifies `src/ipc/bindings.ts` as a single auto-generated file (not a directory). The corrected path `src/ipc/bindings.ts` ensures only that file is excluded from linting, while any other files in `src/ipc/` remain checked.

### globals.css rename (Item 9)
KB §4 specifies `src/styles/globals.css` (with the `s`). The existing file was named `global.css` (without the `s`). The rename brings the filename into conformance with the KB. The import in `src/main.tsx` is updated to match.

### Stub files (Item 10)
KB §4 defines the full frontend file layout. Missing stub files cause import resolution failures as downstream tasks (T2.x – T5.x) reference them. Each stub contains a comment pointing to the relevant KB section and a minimal export (either `export {};` for TS modules or a named default function returning `null` for React components). This satisfies TypeScript's module resolution and Biome's no-empty-file rule without implementing any logic.

### Plugin registration in lib.rs (Item 11)
`tauri_plugin_dialog` and `tauri_plugin_fs` were declared as dependencies in `Cargo.toml` but not registered in the Tauri builder chain in `lib.rs`. Unregistered plugins compile but their commands are not available at runtime — the frontend would get "unknown command" errors. Adding `.plugin(tauri_plugin_dialog::init())` and `.plugin(tauri_plugin_fs::init())` to the builder chain activates them. The `.expect()` message was also updated to be actionable.

### logging.rs module (Item 12)
Already present from the prior task. No change needed — confirmed in place.

---

## Files touched

| File | Change |
|---|---|
| `package.json` | Move `diff2html` from `dependencies` to `devDependencies` |
| `pnpm-lock.yaml` | Updated by pnpm |
| `src-tauri/tauri.conf.json` | Remove `'unsafe-inline'` from `style-src` |
| `src-tauri/capabilities/default.json` | Replace `opener:default` with `opener:allow-open-url` + `opener:allow-open-path` |
| `.github/workflows/build.yml` | Add `permissions: contents: read` after `on:` block |
| `src-tauri/Cargo.toml` | Change `crate-type` to `["cdylib", "rlib"]`; add `tauri-plugin-fs = "2"` |
| `src/styles/global.css` | Deleted (renamed to `globals.css`) |
| `src/styles/globals.css` | Created: renamed from `global.css`; `*` scrollbar selector changed to `html` |
| `src/main.tsx` | Update import from `./styles/global.css` to `./styles/globals.css` |
| `vite.config.ts` | Add `optimizeDeps: { include: ["lucide-react"] }` |
| `biome.json` | Fix ignore from `src/ipc/bindings/` to `src/ipc/bindings.ts` |
| `src-tauri/src/lib.rs` | Add `tauri_plugin_dialog::init()` and `tauri_plugin_fs::init()`; update `.expect()` message |
| `src/stores/ui.ts` | New stub |
| `src/stores/toasts.ts` | New stub |
| `src/stores/liveRuns.ts` | New stub |
| `src/ipc/commands.ts` | New stub |
| `src/ipc/events.ts` | New stub |
| `src/hooks/useProjects.ts` | New stub |
| `src/hooks/useGitStatus.ts` | New stub |
| `src/hooks/useSequences.ts` | New stub |
| `src/hooks/useRunHistory.ts` | New stub |
| `src/hooks/useLiveRun.ts` | New stub |
| `src/hooks/useUsage.ts` | New stub |
| `src/hooks/useSettings.ts` | New stub |
| `src/utils/format.ts` | New stub |
| `src/utils/markdown.ts` | New stub |
| `src/components/ProjectCard.tsx` | New stub |
| `src/components/GitStatusBadge.tsx` | New stub |
| `src/components/RunOutcomeBadge.tsx` | New stub |
| `src/components/LaunchModal.tsx` | New stub |
| `src/components/TagEditorPopover.tsx` | New stub |
| `src/components/Toast.tsx` | New stub |
| `src/components/RateLimitPill.tsx` | New stub |
| `src/components/ContextMenu.tsx` | New stub |
| `src/components/EventBlock/index.tsx` | New stub |
| `src/components/EventBlock/AssistantTextBlock.tsx` | New stub |
| `src/components/EventBlock/ThinkingBlock.tsx` | New stub |
| `src/components/EventBlock/ToolCallBlock.tsx` | New stub |
| `src/components/EventBlock/DiffBlock.tsx` | New stub |
| `src/components/EventBlock/StepFailedBlock.tsx` | New stub |
| `src/components/EventBlock/UserInputBlock.tsx` | New stub |
| `src/components/EventBlock/SystemBlock.tsx` | New stub |

---

## Decisions made within task scope

- Component stubs return `null` (valid React element) rather than an empty fragment `<></>`. Both are accepted by TypeScript and React; `null` is lighter and more obviously a no-op.
- Hook and store stubs use `export {};` (the minimal valid TS module with no exports) rather than exporting a `const` placeholder. This avoids inventing function signatures that upstream contracts have not yet defined.
- `tauri-plugin-shell = "2"` was already in `Cargo.toml` from a prior task. It was left in place — removing it is out of scope for this task (KB §2.6 notes it is not used in v1, but removing it is a separate cleanup decision for the Architect).
- EventBlock stub filenames follow the task instruction exactly (`AssistantTextBlock`, `DiffBlock`, etc.) which differ slightly from KB §4's names (`AssistantBlock`, `FileEditBlock`, `ToolResultBlock`). The KB names are the authoritative ones; the stubs created here are placeholders only. Downstream T5.x tasks will rename/replace them according to KB §4.

---

## How to test / verify

```sh
pnpm install --frozen-lockfile   # must complete without errors
pnpm lint:ci                     # must show "No fixes applied"
pnpm typecheck                   # must produce no output (exit 0)
pnpm test                        # must show 4 passed
```

To verify Rust compiles with the new plugin registrations (requires Rust toolchain):
```sh
cargo check --manifest-path src-tauri/Cargo.toml
```
