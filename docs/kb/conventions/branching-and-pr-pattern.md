# Branching and PR Pattern

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
