---
project: Dev Dashboard
generated: 2026-06-13
commit: f9895e8
phase: v1 build — Epic 4 (Run Execution) in progress
task: —
overall: 45%
---

# Project Status — Dev Dashboard

Local Tauri 2 desktop app (Rust + React/TS) to manage projects and launch Claude Code sequences.
Seeded by project-adoption (sequence 14) on 2026-06-13. Updated automatically by `status-updater` on each Task merge to `master`.

## Features

Epic-level rollup. Per-Task detail lives in `docs/epics/`. Status: ✅ done · 🔶 in-progress · ⬜ pending.

| Epic | Feature | Status | Tests | Notes |
|---|---|---|---|---|
| 0 | Bootstrap & plumbing | ✅ done | ✅ | Scaffold, IPC seam, tracing init. Infra gaps T0.CI-1/2/3 open. |
| 1 | Settings & CLI detection | ✅ done | ✅ | S-01 Setup + S-07 Settings built; `useSettings`, `cli_watcher`. |
| 2 | Project registry & git status | ✅ done | ✅ | S-02 Dashboard, `GitPoller`, tags+filter. T2.9 (tag limit) open. |
| 3 | Sequences | ✅ done | ✅ | `SequenceLoader`, S-03 sequences panel data. |
| 4 | Run execution core | 🔶 in-progress | 🔶 | T4.1–T4.6 merged (parser/manager/transcript/orphan/retention). T4.7 step-failure on branch; T4.8 design resolved. |
| 5 | Run UI (live + historical) | ⬜ pending | ⬜ | S-03/04/05 are stubs. T5.10 (quick-run dispatch) added in adoption. |
| 6 | Toasts & notifications | ⬜ pending | ⬜ | `Toast` component + `toasts` store exist; run-event wiring pending. |
| 7 | Usage / rate-limit | ⬜ pending | ⬜ | `RateLimitPill` + `usage/mod.rs` are stubs. FR-7 designed. |
| 8 | Polish & cross-cutting | ⬜ pending | ⬜ | Depends on Epics 2–7. |
| 9 | Observability | ⬜ pending | ⬜ | Instrumentation PARTIAL (see `docs/monitoring.md`). T9.1–T9.5. |

## Architecture checklist

Rust core components (`docs/kb/system-design.md` §1.2). ☑ = present & functional · 🔶 = present, incomplete · ☐ = stub/absent.

- [x] ProjectRegistry — `projects/mod.rs`
- [x] ProjectScanner — `projects/scanner.rs`
- [x] SequenceLoader — `sequences/mod.rs`
- [x] GitPoller — `projects/git.rs`
- [x] SettingsStore — `settings/mod.rs`
- [x] OrphanReaper — `runs/orphan.rs` (T4.5)
- [x] RetentionPruner — `runs/retention.rs` (T4.6)
- [x] TranscriptWriter — `runs/transcript.rs` (T4.2)
- [x] WindowFocusBridge — focus/blur poll gating
- [ ] RunManager — `runs/manager.rs` 🔶 in-progress (Epic 4)
- [ ] RunSession — `runs/session.rs` 🔶 in-progress
- [ ] EventParser — `runs/parser.rs` 🔶 in-progress (heuristic patterns)
- [ ] UsageProbe — `usage/mod.rs` ☐ stub (Epic 7)

## Dimensions

Rough seed estimates (status-updater recomputes from Task inputs on merge):

| Dimension | % | Note |
|---|---|---|
| Functionality | 45% | Epics 0–3 done; 4 in progress; 5–9 pending. |
| Tests | 55% | FE vitest + Rust unit/integration on shipped modules; UI screens 3/4/5 untested (stubs). |
| Documentation | 95% | Requirements/UI/UX/KB/epics/devops/monitoring all canonical post-adoption. |
| Observability | 30% | Logging scaffold + some component logs; full schema pending (Epic 9). |
| CI/CD | 90% | ci.yml + build.yml live; footer-validation workflow gap (T0.CI-1). |

## Risks

- **EventParser fragility** — heuristic interactive-stdout parsing is the most brittle component (system-design §9.1). Patterns must be validated against real CLI output before Epic 4 closes.
- **Step-failure protocol unconfirmed** — T4.8 chose conservative kill+re-invoke; no confirmed stdin token protocol. T4.7 implements it.
- **Hook wiring gaps** — `update-status.sh` / `log-usage.sh` not previously wired (T0.CI-2/3); post-merge status automation only active after adoption installs it.

<!-- manual:start -->
## Next

_Manual zone — status-updater never edits below this line._

- Land T4.7 (step-failure command) and complete Epic 4.
- Close infra gaps: T0.CI-1 (validate-task-footer workflow), T0.CI-2/3 (wire hooks).
- Implement T2.9 (tag 32-char limit, UI + backend).
- Verify remaining adoption-assumptions in `docs/requirements.md` (FR-1.6, FR-7) and UI ADOPT-04/05.
<!-- manual:end -->
