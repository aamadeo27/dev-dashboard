# Epic 4 — Run Execution Core

The heart. Unblocks live and historical run views.

---

## Dependency graph & parallelism plan

Wave 1 (parallel): T4.1, T4.2
Wave 2 (single): T4.3
Wave 3 (parallel): T4.4, T4.5, T4.6, T4.8
Wave 4 (single): T4.7

## Notes from original epic doc (preserved)

### Dependency Graph and Parallelism Plan

External deps (T0.2, T1.1, T1.2, T2.1) are already shipped, so Epic 4 may start immediately. The graph below shows only intra-epic edges; external prerequisites are noted as `[ext]` leaves where they gate a task.

```
[ext: T0.2 done]
   |
   +--T4.1 ----+
   |           |
   +--T4.2 ----+
               |
              T4.3 <-- [ext: T1.2 done]
               |
       +-------+------+--------------------+----------------+
       |       |      |                    |                |
      T4.4   T4.5   T4.6                  T4.8             (also T4.5 <- [ext: T2.1 done])
                                           |                (also T4.6 <- [ext: T1.1 done])
                                          T4.7
```

Edge legend: an arrow from A to B means "B depends on A". T4.8 also depends on T4.1 directly, but that edge is implied transitively via T4.3 in the picture above (T4.1 -> T4.3 -> T4.8).

### Wave Plan

Tasks within the same wave have no intra-epic dependencies on each other and can be picked up in parallel by separate Sonnet sessions.

| Wave | Tasks (parallel) | Gate |
|---|---|---|
| W1 | **T4.1**, **T4.2** | external deps done; no intra-epic predecessor |
| W2 | **T4.3** | needs T4.1 + T4.2 merged |
| W3 | **T4.8** | needs T4.3 (and T4.1) — must run a real interactive session to pin step-failure protocol |
| W4 | **T4.4**, **T4.5**, **T4.6**, **T4.7** | all gated on T4.3; T4.7 additionally gated on T4.8 (resolved in W3) |

Critical path: **T4.1 -> T4.3 -> T4.8 -> T4.7** (four serial tasks). T4.2 is off the critical path but must land before T4.3 begins. T4.4/T4.5/T4.6 finish whenever W4 finishes; none extend the critical path beyond T4.7.

### Serialization Constraints

Even within a wave, a few tasks touch the same files and should be ordered by the Coder (or merged behind a short-lived feature branch) to avoid conflicts:

- **W4 — T4.4 and T4.7 both modify `RunSession` stdin-write paths**. T4.4 prepends attached-md content on the first write; T4.7 writes step-failure response tokens (or triggers re-invoke per T4.8 outcome). Land T4.4 first (smaller surface, only first-write hook), then layer T4.7 on top. T4.5 and T4.6 are isolated (startup/timer modules) and parallelize cleanly against both.
- **W4 — T4.7 cannot start until T4.8's KB §9 update is merged**, because T4.7's acceptance criteria are rewritten by T4.8. Treat T4.8 as a documentation+spike task that unblocks T4.7 implementation.
- **W2 — T4.3 is a single-task wave by design**: it owns `RunManager` / `RunSession` wiring and integrates T4.1 (parser) + T4.2 (writer). Splitting it risks contract drift on the `run:event` payload shape.
- **W1 — T4.1 and T4.2 are fully independent** (different modules: `runs/parser/` vs. `runs/writer/`) and share no files; safe to run in parallel without coordination.
