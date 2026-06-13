# Epic 0 — Project Bootstrap and Plumbing

Foundation. Blocks everything.

---

## Dependency graph & parallelism plan

Wave 1 (parallel): T0.1, T0.CI-1, T0.CI-2, T0.CI-3, T0.CI-4
Wave 2 (parallel): T0.2, T0.5
Wave 3 (parallel): T0.3, T0.6
Wave 4 (single): T0.4

## Notes from original epic doc (preserved)

### Infra Gap Tasks (added 2026-06-13, adoption audit)

The following tasks were identified during DevOps adoption audit. They address gaps between the DevOps plan
(`docs/devops.md`) and the actual on-disk state. None of these block feature work; they are housekeeping.

---
