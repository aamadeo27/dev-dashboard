# Epics: Dev Dashboard v1

**Date**: 2026-05-18
**Companion doc**: `../../docs/kb/README.md` (read first)

## Conventions

- Each task is tagged `[frontend]`, `[backend]`, or `[shared]`.
- Each task has: **Description**, **Acceptance Criteria**, **Dependencies** (task IDs).
- Tasks are sized for a single Sonnet coding session (~2–4h).
- Order is dependency-respecting; within a level, `[frontend]` and `[backend]` may run in parallel.
- "KB §X.Y" references the Knowledge Base section.

## Files

| File | Epic |
|---|---|
| [epic-0-bootstrap.md](epic-0-bootstrap.md) | Epic 0 — Project Bootstrap and Plumbing |
| [epic-1-settings-cli.md](epic-1-settings-cli.md) | Epic 1 — Settings and CLI Detection |
| [epic-2-project-registry-git.md](epic-2-project-registry-git.md) | Epic 2 — Project Registry and Git Status ✓ done |
| [epic-3-sequences.md](epic-3-sequences.md) | Epic 3 — Sequences ✓ done |
| [epic-4-run-execution.md](epic-4-run-execution.md) | Epic 4 — Run Execution Core |
| [epic-5-run-ui.md](epic-5-run-ui.md) | Epic 5 — Run UI (Live and Historical) |
| [epic-6-toasts.md](epic-6-toasts.md) | Epic 6 — Toasts and Notifications |
| [epic-7-usage.md](epic-7-usage.md) | Epic 7 — Usage / Rate Limit |
| [epic-8-polish.md](epic-8-polish.md) | Epic 8 — Polish and Cross-Cutting |
| [epic-9-observability.md](epic-9-observability.md) | Epic 9 — Observability |

---

## Dependency Graph (high-level)

```
T0.1
 |--T0.2--T0.3--T0.4
 |    |    |
 |    |    +--T0.6
 |    |
 |--T0.5
 |
 +--Epic 1 (T1.x)  -- unblocks Setup, Settings
 +--Epic 2 (T2.x)  -- unblocks Dashboard
 +--Epic 3 (T3.x)  -- unblocks Project Detail
 +--Epic 4 (T4.x)  -- core: parser/manager/writer/orphan/retention
 +--Epic 5 (T5.x)  -- depends on Epic 4 + Epic 3
 +--Epic 6 (T6.x)  -- depends on T4.3
 +--Epic 7 (T7.x)  -- depends on T1.2
 +--Epic 8 (T8.x)  -- polish, depends on Epics 2-7
 +--Epic 9 (T9.x)  -- observability, depends on T0.6 + Epics 2/4/6/7
```

## Parallelization Notes

After Epic 0 lands:

- **Track A (backend)**: T1.1 -> T1.2 -> T2.1 -> T2.2 -> T2.3 -> T3.1 -> T4.1 -> T4.2 -> T4.3 -> T4.8 -> T4.4/T4.5/T4.6/T4.7 -> T5.5 -> T6.2 -> T7.1 -> T8.3/T8.4 -> T8.7.
- **Track B (frontend)**: T1.3 -> T1.4 -> T2.4 -> T2.5 -> T2.6 -> T2.7 -> T3.2 -> T5.1 -> T5.2 -> T5.3 -> T5.4 -> T5.6 -> T5.7 -> T5.8 -> T5.9 -> T6.1 -> T6.3 -> T7.2 -> T8.1/T8.2/T8.5.

Frontend track can develop against typed mocks for any backend command before its Rust impl lands — the IPC contract is the seam.

---

## Coverage Check

Every requirement from `../requirements.md` and every screen from `../ui-ux.md` is covered:

| Req / Screen | Task(s) |
|---|---|
| FR-1.1 register project | T2.1, T2.6 |
| FR-1.2 remove project | T2.1, T2.7 |
| FR-1.3 card display | T2.5, T2.6 |
| FR-1.4 git polling | T2.3, T8.3 |
| FR-1.5 missing state | T2.1, T2.5 |
| FR-2.1 sequences as files | T3.1 |
| FR-2.2 browse sequences | T3.1, T3.2 |
| FR-2.3 launch | T4.3, T5.8 |
| FR-2.4 attach .md | T4.4, T5.8 |
| FR-2.5 unlimited concurrency | T4.3 |
| FR-3.1 spawn CLI | T4.3 |
| FR-3.2 CLI missing | T1.2, T1.3 |
| FR-3.3 parsed rendering | T4.1, T5.1, T5.3 |
| FR-3.4 stop | T4.3, T5.3 |
| FR-3.5 orphan detection | T4.5 |
| FR-3.6 state transitions | T4.3 |
| FR-4.1/4.2 transcript persistence | T4.2 |
| FR-4.3 browse past runs | T5.5, T5.7 |
| FR-4.4 retention | T4.6, T1.4 |
| FR-5.1-5.4 toasts | T6.1, T6.2, T6.3 |
| FR-6.1/6.2 settings | T1.1, T1.4 |
| Step failure handling | T4.7, T5.4 |
| Usage status bar | T7.1, T7.2 |
| S-01 Setup | T1.3 |
| S-02 Dashboard | T2.5, T2.6, T2.7, T7.2 |
| S-03 Project Detail | T5.7 |
| S-04 Run Live | T5.3, T5.4, T5.9 |
| S-05 Run Historical | T5.6 |
| S-06 Launch Modal | T5.8 |
| Quick-run dispatch (⚡ button) | T2.5, T5.7, T5.8, T5.10 |
| S-07 Settings | T1.4 |
| S-08 Tag Editor | T2.7 |
| S-09 Toasts | T6.1, T6.2, T6.3 |
