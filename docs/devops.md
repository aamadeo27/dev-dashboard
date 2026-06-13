# DevOps Plan: Dev Dashboard

**Date**: 2026-05-18 (relocated 2026-06-13)
**App**: Tauri 2 desktop app — Rust + React + TS + Vite, pnpm workspace, cross-platform (Windows/macOS/Linux).
**Distribution**: Personal use only. No app store, no code signing for external distribution, no auto-update service in v1.

> [adoption-assumption] Canonical relocation from `.claude/devops.md`. Plan text preserved verbatim; drift corrections
> from audit (2026-06-13) are marked inline.

---

## 1. No Secrets

This repo manages **zero secrets**. There are no API keys, tokens, or credentials owned by the application.

- The `claude` CLI handles its own authentication externally (managed by Claude Code, not this repo).
- No `.env` files, no secret manager integration, no credentials injected at build or runtime.
- Nothing needs to be gitignored for security reasons beyond standard OS build artifacts.
- If this changes in a future version, see the Secrets Management section in `docs/kb/conventions/secrets.md`.

> [adoption-assumption] `.env.local` exists on disk (`DEV_DASHBOARD_LOG=debug`, `DEV_DASHBOARD_CONFIG_DIR=.dev-data`).
> It is gitignored via `.gitignore` rule `*.env*.local` and has never appeared in git history. No secret concern.
> This file supports the `pnpm dev:local` convenience script (not listed in the original plan but present in `package.json`).

---

## 2. Local Dev Workflow

All commands run from the repo root. Requires: `pnpm`, `cargo`/`rustup` with the current stable toolchain, and Tauri CLI (`cargo install tauri-cli` or `pnpm tauri`).

### pnpm scripts (defined in `package.json`)

| Script | Command | Purpose |
|---|---|---|
| `pnpm dev` | `pnpm tauri dev` | Start Vite dev server + Tauri window with hot reload |
| `pnpm dev:local` | `cross-env DEV_DASHBOARD_LOG=debug DEV_DASHBOARD_CONFIG_DIR=.dev-data pnpm tauri dev` | Dev with debug logging + isolated data dir (reads `.env.local`) |
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

### Claude Code hooks (`.claude/hooks/`)

Three hooks live in `.claude/hooks/` for Claude Code integration. These are separate from git hooks:

| Hook file | Purpose | Wire point |
|---|---|---|
| `validate-task-footer.sh` | Validates `Task: T<n>.<m>` and `Notes:` footers on integration commits | `commit-msg` hook (local) or CI workflow |
| `update-status.sh` | Triggers `status-updater` agent on merge to `main` to update `PROJECT_STATUS.md` | `post-merge` git hook (manual install to `.git/hooks/`) |
| `log-usage.sh` | Logs Claude token usage to `DevTeam.log` | Claude Code `Stop`/`SubagentStop` hook in `.claude/settings.json` |

> [adoption-assumption] `pnpm prepare` sets `core.hooksPath .githooks` but `.githooks/` only contains `pre-commit`.
> `update-status.sh` must be manually linked to `.git/hooks/post-merge` to be active (not automated by `prepare`).
> `log-usage.sh` requires an entry in `.claude/settings.json` which does not currently exist (only
> `.claude/settings.local.json` with permissions). See gap tasks T0.CI-1, T0.CI-2, T0.CI-3.

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

### Squash-merge commit footer convention

Squash-merge commits targeting `main` must include:

```
Task: T<n>.<m>
Notes: <one sentence summary>
```

This footer is validated by `.claude/hooks/validate-task-footer.sh` and is required by `update-status.sh` to trigger the status-updater agent.

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

> [adoption-assumption] The on-disk `ci.yml` improves on the plan spec with: SHA-pinned action refs (security
> hardening), `permissions: contents: read` at workflow level, and a concurrency group
> (`ci-${{ github.ref }}`, cancel-in-progress) to abort superseded runs. These are confirmed present in
> `.github/workflows/ci.yml`. The plan YAML below reflects the actual file.

Runs on `ubuntu-latest`, `macos-latest`, `windows-latest` for the full matrix only on PRs to main. On plain branch pushes (not PR), run only on `ubuntu-latest` to save minutes. The `if:` condition on the job controls this: `github.event_name == 'pull_request' || matrix.os == 'ubuntu-latest'`.

```yaml
# .github/workflows/ci.yml — actual content (SHA pins abbreviated for readability)
name: CI

on:
  push:
    branches-ignore: []   # all branches
  pull_request:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  check:
    name: Check (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    if: github.event_name == 'pull_request' || matrix.os == 'ubuntu-latest'
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]

    steps:
      - uses: actions/checkout@v4  # SHA-pinned on disk

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable  # SHA-pinned on disk
        with:
          components: clippy, rustfmt

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2  # SHA-pinned on disk
        with:
          workspaces: src-tauri

      - name: Install pnpm
        uses: pnpm/action-setup@v4  # SHA-pinned on disk
        with:
          version: 9

      - name: Setup Node
        uses: actions/setup-node@v4  # SHA-pinned on disk
        with:
          node-version: 22
          cache: pnpm

      - name: Install Tauri system deps (Linux only)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Install JS dependencies
        run: pnpm install --frozen-lockfile

      - name: Biome lint (JS/TS)
        run: pnpm lint:ci

      - name: TypeScript typecheck
        run: pnpm typecheck

      - name: rustfmt check
        run: cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

      - name: Clippy
        run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

      - name: Frontend tests (Vitest)
        run: pnpm test

      - name: Rust tests
        run: cargo test --manifest-path src-tauri/Cargo.toml

      - name: Regenerate ts-rs bindings
        run: pnpm bindings

      - name: Assert bindings unchanged
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

> [adoption-assumption] The on-disk `build.yml` diverges from the plan spec in two ways:
> 1. Matrix key is `artifact` (not `artifact-suffix`); artifact naming is identical.
> 2. The `release` job uses `gh release create` CLI (with `GH_TOKEN: ${{ github.token }}`) instead of the
>    `softprops/action-gh-release@v2` action. Functionally equivalent; the CLI approach avoids a third-party
>    action dependency.
> Both divergences are improvements. The canonical doc reflects the actual file.

Triggered on version tags (`v1.0.0`, `v1.2.3`, etc.). Builds platform-native installers and uploads them to a GitHub Release.

```yaml
# .github/workflows/build.yml — actual content (SHA pins abbreviated for readability)
name: Build

on:
  push:
    tags:
      - 'v[0-9]+.[0-9]+.[0-9]+'

permissions:
  contents: read

jobs:
  build:
    name: Build (${{ matrix.artifact }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: linux-x64
          - os: macos-13
            target: x86_64-apple-darwin
            artifact: macos-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: macos-arm64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: windows-x64

    steps:
      - uses: actions/checkout@v4  # SHA-pinned on disk

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable  # SHA-pinned on disk

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2  # SHA-pinned on disk
        with:
          workspaces: src-tauri

      - name: Install pnpm
        uses: pnpm/action-setup@v4  # SHA-pinned on disk
        with:
          version: 9

      - name: Setup Node
        uses: actions/setup-node@v4  # SHA-pinned on disk
        with:
          node-version: 22
          cache: pnpm

      - name: Install Tauri system deps (Linux only)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Install JS dependencies
        run: pnpm install --frozen-lockfile

      - name: Build Tauri app
        run: pnpm tauri build
        env:
          TAURI_PRIVATE_KEY: ""
          TAURI_KEY_PASSWORD: ""

      - name: Upload build artifacts
        uses: actions/upload-artifact@v4  # SHA-pinned on disk
        with:
          name: dev-dashboard-${{ matrix.artifact }}-${{ github.ref_name }}
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
      - uses: actions/checkout@v4  # SHA-pinned on disk

      - name: Download all artifacts
        uses: actions/download-artifact@v4  # SHA-pinned on disk
        with:
          path: artifacts/

      - name: Create draft GitHub Release
        run: |
          gh release create "${{ github.ref_name }}" \
            --draft \
            --generate-notes \
            --title "${{ github.ref_name }}" \
            artifacts/**/*
        env:
          GH_TOKEN: ${{ github.token }}
```

### Artifact naming pattern

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
- **How to run**: `pnpm dev` (`pnpm tauri dev`) or `pnpm dev:local` for isolated data + debug logging.
- **Tauri**: dev mode, points to Vite dev server at `http://localhost:1420`.
- **Rust**: debug profile (`cargo build`), no release optimizations.
- **Log level**: `debug` when `DEV_DASHBOARD_LOG=debug` is set in the shell. `pnpm dev:local` sets this automatically.
- **Data isolation**: `pnpm dev:local` sets `DEV_DASHBOARD_CONFIG_DIR=.dev-data` for isolation. Plain `pnpm dev` uses the same OS config dir as the production binary.
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

`.github/dependabot.yml` is present and matches the plan exactly:

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

The app's runtime monitoring is fully local (see KB §7 / `docs/monitoring.md`). CI monitoring is minimal:

- **Build status**: GitHub Actions badges in README (add after repo is created).
- **Failed CI notifications**: GitHub's built-in email notification on failed workflow runs. No additional alerting needed.
- **Log level in CI**: `RUST_LOG=info` is set implicitly; no `tracing-appender` in test runs (stdout only via `tracing-subscriber` env filter).

No remote telemetry, no Datadog, no Sentry. The app explicitly excludes remote telemetry (NFR-8).

---

## 12. Gaps and Known Issues

The following gaps exist between the plan and current on-disk state. Each has a corresponding task in `docs/epics/epic-0-bootstrap/`.

| Gap | Task |
|---|---|
| No CI workflow enforces `validate-task-footer.sh` on PRs | T0.CI-1 |
| `update-status.sh` not wired as a git `post-merge` hook (not installed by `pnpm prepare`) | T0.CI-2 |
| `log-usage.sh` not wired — `.claude/settings.json` missing | T0.CI-3 |
| KB `branching-and-pr-pattern.md` references stale path `.claude/devops.md` | T0.CI-4 |
