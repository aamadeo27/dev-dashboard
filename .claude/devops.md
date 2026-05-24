# DevOps Plan: Dev Dashboard

**Date**: 2026-05-18
**App**: Tauri 2 desktop app — Rust + React + TS + Vite, pnpm workspace, cross-platform (Windows/macOS/Linux).
**Distribution**: Personal use only. No app store, no code signing for external distribution, no auto-update service in v1.

---

## 1. No Secrets

This repo manages **zero secrets**. There are no API keys, tokens, or credentials owned by the application.

- The `claude` CLI handles its own authentication externally (managed by Claude Code, not this repo).
- No `.env` files, no secret manager integration, no credentials injected at build or runtime.
- Nothing needs to be gitignored for security reasons beyond standard OS build artifacts.
- If this changes in a future version, see the Secrets Management section in `docs/kb/conventions/secrets.md`.

---

## 2. Local Dev Workflow

All commands run from the repo root. Requires: `pnpm`, `cargo`/`rustup` with the current stable toolchain, and Tauri CLI (`cargo install tauri-cli` or `pnpm tauri`).

### pnpm scripts (defined in `package.json`)

| Script | Command | Purpose |
|---|---|---|
| `pnpm dev` | `pnpm tauri dev` | Start Vite dev server + Tauri window with hot reload |
| `pnpm build` | `pnpm tauri build` | Production build for the current platform |
| `pnpm preview` | `vite preview` | Preview the Vite frontend build in a browser (no Tauri) |
| `pnpm lint` | `biome check --write src/` | Lint + format JS/TS with Biome |
| `pnpm lint:ci` | `biome ci src/` | Lint check only, no writes (used in CI) |
| `pnpm typecheck` | `tsc --noEmit` | TypeScript type check without emit |
| `pnpm test` | `vitest run` | Run all frontend unit tests once |
| `pnpm test:watch` | `vitest` | Vitest in watch mode for local dev |
| `pnpm bindings` | `cross-env TS_RS_EXPORT_DIR=../src/ipc/ cargo test --features export-bindings --manifest-path src-tauri/Cargo.toml` | Regenerate `src/ipc/bindings.ts` from Rust ts-rs derives (`TS_RS_EXPORT_DIR` controls output path) |
| `pnpm prepare` | `git config core.hooksPath .githooks \|\| true` | Activate pre-commit hooks (run once after clone) |

### Cargo commands (run from `src-tauri/` or root with `--manifest-path`)

| Command | Purpose |
|---|---|
| `cargo clippy -- -D warnings` | Rust lint, treat all warnings as errors |
| `cargo fmt --check` | Rust format check (CI); omit `--check` to auto-format locally |
| `cargo fmt` | Auto-format Rust source |
| `cargo test` | Run Rust unit + integration tests |
| `cargo test --features export-bindings` | Runs binding export (triggered by `pnpm bindings`) |

### Checking bindings are up to date

Run `pnpm bindings` then `git diff --exit-code src/ipc/bindings.ts`. A non-zero exit means the bindings are stale. CI gates on this check.

### Pre-commit hooks

Run `pnpm prepare` once after cloning to activate the hooks in `.githooks/`. The pre-commit hook runs `pnpm lint:ci` and `pnpm typecheck`. Bindings freshness requires cargo and is checked in CI only.

---

## 3. Branching Strategy

Single developer. The goal is a disciplined history and a reliable main branch, not process overhead.

### Branch types

| Prefix | Use |
|---|---|
| `feat/<ticket-id>-short-description` | New feature work (maps to an Epic task, e.g. `feat/T0.1-scaffold`) |
| `fix/<ticket-id>-short-description` | Bug fix |
| `chore/<short-description>` | Dependency bumps, tooling, CI config, non-functional changes |
| `docs/<short-description>` | Documentation only |
| `refactor/<short-description>` | Code restructuring without behavior change |

### Flow

```
main (always releasable)
  |
  +--feat/T0.1-scaffold
  |    (work, commits)
  |    pnpm lint && cargo clippy && pnpm test && cargo test
  +--PR (self-review) --> squash-merge to main
```

- `main` is the only long-lived branch. No `develop`, no `release` branch in v1.
- Feature branches are short-lived (ideally < 1 day per task).
- **All merges to main go through a PR**, even solo. This enforces CI to run and gives a clean review record.
- Squash-merge by default so main has one commit per task. Keep the PR number in the merge commit message.
- Delete the feature branch after merge.

### PR conventions (solo)

- PR title: `<type>(<scope>): <imperative description>` — e.g. `feat(runs): implement RunManager spawn and session lifecycle`
- PR body must include:
  - **What**: one sentence.
  - **How**: key implementation decisions (2-4 bullets).
  - **Test**: what was tested manually or via automated tests.
  - **Checklist**: `[ ] lint passes`, `[ ] typecheck passes`, `[ ] clippy passes`, `[ ] tests pass`, `[ ] bindings up to date` (if Rust types changed).
- Required CI checks must be green before merging (enforced by GitHub branch protection on `main`).
- No force-push to `main`.

---

## 4. Commit Conventions

Conventional Commits format. Required for automated changelog generation via `git-cliff` or similar.

```
<type>(<scope>): <short description>

[optional body — the "why", not the "what"]

[optional footer — "Closes #12", "BREAKING CHANGE: ..."]
```

### Types

| Type | When |
|---|---|
| `feat` | New user-facing feature |
| `fix` | Bug fix |
| `chore` | Tooling, deps, CI, no production code change |
| `docs` | Documentation only |
| `refactor` | Code change with no behavior change |
| `test` | Adding or fixing tests |
| `perf` | Performance improvement |
| `build` | Changes to build system or external dependencies |

### Scopes (suggested, not enforced)

`runs`, `projects`, `git`, `settings`, `usage`, `ipc`, `frontend`, `parser`, `retention`, `ci`, `deps`

### Rules

- Subject line: imperative mood, no trailing period, max 72 chars.
- Body: wrap at 72 chars, explain *why* not *what*.
- Breaking changes: add `BREAKING CHANGE:` footer or `!` after the type (`feat!:`).

---

## 5. CI Pipeline

### Trigger rules

| Workflow | Triggers |
|---|---|
| `ci.yml` | Push to any branch; PR opened/synchronize/reopened targeting `main` |
| `build.yml` | Push of a tag matching `v[0-9]+.[0-9]+.[0-9]+` |

### `ci.yml` — Check workflow

Runs on `ubuntu-latest`, `macos-latest`, `windows-latest` for the full matrix only on PRs to main. On plain branch pushes (not PR), run only on `ubuntu-latest` to save minutes.

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches-ignore: []   # all branches
  pull_request:
    branches: [main]

jobs:
  check:
    name: Check (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        # On non-PR pushes, only run Linux to save minutes
        # Controlled via if: condition below

    steps:
      - uses: actions/checkout@v4

      # --- Rust toolchain ---
      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      # --- Node / pnpm ---
      - name: Install pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 9

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm

      - name: Install JS dependencies
        run: pnpm install --frozen-lockfile

      # --- Linux system deps for Tauri (WebKit) ---
      - name: Install Tauri system deps (Linux only)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      # --- Lint and type checks ---
      - name: Biome lint (JS/TS)
        run: pnpm lint:ci

      - name: TypeScript typecheck
        run: pnpm typecheck

      - name: rustfmt check
        run: cargo fmt --check --manifest-path src-tauri/Cargo.toml

      - name: Clippy
        run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

      # --- Tests ---
      - name: Frontend tests (Vitest)
        run: pnpm test

      - name: Rust tests
        run: cargo test --manifest-path src-tauri/Cargo.toml

      # --- Bindings freshness ---
      - name: Regenerate ts-rs bindings
        run: pnpm bindings

      - name: Check bindings are committed
        run: git diff --exit-code src/ipc/bindings.ts
```

### Required checks (branch protection on `main`)

All three OS jobs from `ci.yml` must pass before merge is allowed:
- `Check (ubuntu-latest)`
- `Check (macos-latest)`
- `Check (windows-latest)`

---

## 6. Build Pipeline

### `build.yml` — Cross-platform binary release

Triggered on version tags (`v1.0.0`, `v1.2.3`, etc.). Builds platform-native installers and uploads them to a GitHub Release.

```yaml
# .github/workflows/build.yml
name: Build

on:
  push:
    tags:
      - 'v[0-9]+.[0-9]+.[0-9]+'

jobs:
  build:
    name: Build (${{ matrix.platform }})
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: linux-x64
            runner: ubuntu-latest
            artifact-suffix: linux-x64
          - platform: macos-x64
            runner: macos-13         # Intel runner
            artifact-suffix: macos-x64
          - platform: macos-arm64
            runner: macos-latest     # Apple Silicon runner (M1+)
            artifact-suffix: macos-arm64
          - platform: windows-x64
            runner: windows-latest
            artifact-suffix: windows-x64

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 9

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm

      - name: Install JS dependencies
        run: pnpm install --frozen-lockfile

      - name: Install Tauri system deps (Linux only)
        if: matrix.runner == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Build Tauri app
        run: pnpm tauri build
        env:
          # Disable code signing for personal-use builds
          TAURI_PRIVATE_KEY: ''
          TAURI_KEY_PASSWORD: ''

      # --- Collect artifacts ---
      # Linux: .AppImage and .deb in src-tauri/target/release/bundle/
      # macOS: .dmg in src-tauri/target/release/bundle/dmg/
      # Windows: .msi and .exe in src-tauri/target/release/bundle/msi/ and /nsis/

      - name: Upload build artifacts
        uses: actions/upload-artifact@v4
        with:
          name: dev-dashboard-${{ matrix.artifact-suffix }}-${{ github.ref_name }}
          path: |
            src-tauri/target/release/bundle/**/*.AppImage
            src-tauri/target/release/bundle/**/*.deb
            src-tauri/target/release/bundle/**/*.dmg
            src-tauri/target/release/bundle/**/*.msi
            src-tauri/target/release/bundle/**/*.exe
          if-no-files-found: error

  release:
    name: Create GitHub Release
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts/

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          draft: true           # review before publishing
          generate_release_notes: true
          files: artifacts/**/*
```

### Artifact naming pattern

Tauri produces platform-native bundles automatically. GitHub Actions uploads them under the names:

```
dev-dashboard-linux-x64-v1.0.0/
  dev-dashboard_1.0.0_amd64.AppImage
  dev-dashboard_1.0.0_amd64.deb
dev-dashboard-macos-x64-v1.0.0/
  dev-dashboard_1.0.0_x64.dmg
dev-dashboard-macos-arm64-v1.0.0/
  dev-dashboard_1.0.0_aarch64.dmg
dev-dashboard-windows-x64-v1.0.0/
  dev-dashboard_1.0.0_x64-setup.exe
  dev-dashboard_1.0.0_x64_en-US.msi
```

The GitHub Release draft collects all of these under a single `v1.0.0` tag.

---

## 7. Release Process

### Tagging and releasing

1. Ensure `main` is green (all CI checks pass).
2. Bump the version in `src-tauri/Cargo.toml` (`version = "1.0.0"`) and `package.json` (`"version": "1.0.0"`). Also update `src-tauri/tauri.conf.json` (`"version": "1.0.0"` under `package`). Commit as `chore(release): bump version to 1.0.0`.
3. Tag: `git tag -a v1.0.0 -m "v1.0.0"` then `git push origin v1.0.0`.
4. The `build.yml` workflow triggers automatically. Monitor the Actions tab.
5. When all four platform jobs succeed, open the draft GitHub Release, add release notes, and publish.

### Version numbering

Semantic versioning: `MAJOR.MINOR.PATCH`.
- `PATCH`: bug fixes, no new commands or IPC changes.
- `MINOR`: new features, backward-compatible IPC additions.
- `MAJOR`: breaking IPC changes, data model migrations, major UX restructuring.

### Rollback

There is no rollback mechanism in v1 — the app is local-only with no server to roll back. If a build is bad:
- Do not publish the draft Release.
- Fix on main, cut a new patch tag (`v1.0.1`).
- For already-distributed binaries, notify the user (yourself) to download the new build manually.

---

## 8. Environments

This is a local desktop app. There is no server, no deployment target. "Environments" are build modes.

### Dev (local)

- **Purpose**: Active development. Hot reload, debug logging, no optimization.
- **How to run**: `pnpm dev` (`pnpm tauri dev`)
- **Tauri**: dev mode, points to Vite dev server at `http://localhost:1420`.
- **Rust**: debug profile (`cargo build`), no release optimizations.
- **Log level**: `debug` when `DEV_DASHBOARD_LOG=debug` is set in the shell before running.
- **Data isolation**: none enforced — dev uses the same OS config dir as a "production" binary. If you need isolation, set `DEV_DASHBOARD_CONFIG_DIR` to a temp path (if the app supports that override; add it in T0.2 as a dev convenience).
- **Access**: local machine only.

### Prod (release binary)

- **Purpose**: Installed, running binary for personal daily use.
- **How to build**: `pnpm build` (`pnpm tauri build`) or via `build.yml` on tag push.
- **Rust**: release profile (`cargo build --release`) with Tauri's built-in optimizations.
- **Log level**: `info` by default. User can set `DEV_DASHBOARD_LOG=debug` if debugging a problem.
- **Data**: real user data in OS config dir (`~/.config/dev-dashboard/`, `%APPDATA%\dev-dashboard\`, `~/Library/Application Support/dev-dashboard/`).
- **SLOs**: startup time < 2 s (cold), idle RAM < 200 MB (NFR-6). Enforced by manual verification, not automated monitoring.
- **Access**: local machine only.

### CI (GitHub Actions)

- **Purpose**: Automated checks on every push and PR; cross-platform builds on version tags.
- **Data**: ephemeral runner filesystems. No real user data.
- **Secrets**: none. The build requires no credentials.

---

## 9. Dependency Management

### Dependabot configuration

Place at `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: npm
    directory: /
    schedule:
      interval: weekly
      day: monday
    open-pull-requests-limit: 5
    groups:
      tauri-js:
        patterns:
          - "@tauri-apps/*"
      react:
        patterns:
          - "react"
          - "react-dom"
          - "@types/react*"

  - package-ecosystem: cargo
    directory: /src-tauri
    schedule:
      interval: weekly
      day: monday
    open-pull-requests-limit: 5
    groups:
      tauri-rs:
        patterns:
          - "tauri*"
```

### Update policy

| Semver change | Action |
|---|---|
| Patch | Merge after CI passes. No manual review needed. |
| Minor | Read the changelog entry. Merge if no behavioral changes in used APIs. |
| Major | Manual review required. Do not merge automatically. Check migration guides. |

Special caution:
- `tauri` (Rust) and `@tauri-apps/api` (JS) must be bumped together — they are versioned in lockstep.
- `git2` updates may bring libgit2 API changes; review the git2 changelog before merging.
- `ts-rs` updates may change generated TS output format; run `pnpm bindings` and inspect the diff.

---

## 10. Infrastructure

This is a desktop app. There is no server infrastructure, no database service, no container registry, no CDN.

### What does exist

| Component | Where |
|---|---|
| Source code | GitHub repository |
| CI/CD | GitHub Actions (free tier is sufficient for a personal repo) |
| Release binaries | GitHub Releases (attached to version tags) |
| User data (runtime) | Local OS filesystem per-user paths (see KB §3, §4) |
| Logs (runtime) | `<os_config_dir>/dev-dashboard/logs/` — local, never shipped |

### GitHub repository settings

- **Default branch**: `main`
- **Branch protection on `main`**:
  - Require status checks to pass before merging: `Check (ubuntu-latest)`, `Check (macos-latest)`, `Check (windows-latest)`
  - Require branches to be up to date before merging
  - Do not allow force pushes
  - Do not allow deletions
- **Actions permissions**: allow all actions (no restrictions needed for a personal repo)

---

## 11. Monitoring Hooks (CI)

The app's runtime monitoring is fully local (see KB §7). CI monitoring is minimal:

- **Build status**: GitHub Actions badges in README (add after repo is created).
- **Failed CI notifications**: GitHub's built-in email notification on failed workflow runs. No additional alerting needed.
- **Log level in CI**: `RUST_LOG=info` is set implicitly; no `tracing-appender` in test runs (stdout only via `tracing-subscriber` env filter).

No remote telemetry, no Datadog, no Sentry. The app explicitly excludes remote telemetry (NFR-8).
