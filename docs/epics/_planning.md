# Epics — cross-epic planning

**Date**: 2026-05-18 (relocated from `README.md` during adoption, 2026-06-13, when epics moved to the canonical folder layout)
**Companion doc**: `../kb/README.md` (read first)

Per-epic wave plans now live in each `epic-*/DESCRIPTION.md` under `## Dependency graph & parallelism plan`. This file holds the **cross-epic** picture.

## Conventions

- Each task is tagged `frontend`, `backend`, `shared`, or `infra` (in the task file's `**Tag**:` line).
- Each task file has: title (`#` heading), `**Tag**`, `deps:` line, Description, Acceptance Criteria, and a kb-refs block.
- Tasks are sized for a single Sonnet coding session (~2–4h).
- Order is dependency-respecting; within a wave, `frontend` and `backend` may run in parallel.
- "KB §X.Y" references the Knowledge Base section.

## Dependency Graph (high-level, inter-epic)

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

## Coverage Check

Every requirement from `../requirements.md` and every screen from `../ui-ux.md` is covered:

| Req / Screen | Task(s) |
|---|---|
| FR-1.1 register project | T2.1, T2.6 |
| FR-1.2 remove project | T2.1, T2.7 |
| FR-1.3 card display | T2.5, T2.6 |
| FR-1.4 git polling | T2.3, T8.3 |
| FR-1.5 missing state | T2.1, T2.5 |
| FR-1.6 tags + filter | T2.1, T2.6, T2.7, T2.9 |
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
| FR-3.7 step failure handling | T4.7, T4.8, T5.4 |
| FR-4.1/4.2 transcript persistence | T4.2 |
| FR-4.3 browse past runs | T5.5, T5.7 |
| FR-4.4 retention | T4.6, T1.4 |
| FR-5.1-5.4 toasts | T6.1, T6.2, T6.3 |
| FR-6.1/6.2 settings | T1.1, T1.4 |
| FR-7 usage monitoring | T7.1, T7.2 |
| Quick-run dispatch (⚡ button) | T2.5, T5.7, T5.8, T5.10 |
| S-01 Setup | T1.3 |
| S-02 Dashboard | T2.5, T2.6, T2.7, T7.2 |
| S-03 Project Detail | T5.7 |
| S-04 Run Live | T5.3, T5.4, T5.9 |
| S-05 Run Historical | T5.6 |
| S-06 Launch Modal | T5.8 |
| S-07 Settings | T1.4 |
| S-08 Tag Editor | T2.7 |
| S-09 Toasts | T6.1, T6.2, T6.3 |
