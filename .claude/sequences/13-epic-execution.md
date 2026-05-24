# Sequence: Epic execution

Goal: drive every Task in an Epic to completion, running independent Tasks in parallel.

## When to use
- Architect has produced `docs/epics/` with one file per Epic and a `README.md` index.
- You're ready to implement one full Epic.
- For a single ad-hoc Task, use `04-task-feature.md` directly. For a brand-new change request, use `05-evolution.md`.

## Inputs
- The target Epic, identified by id (e.g. `002`) or filename (`002-<slug>.md`).
- `docs/epics/` folder with one file per Epic and `docs/epics/README.md` as index.
- Knowledge Base (contracts, conventions, patterns).

If the Epic argument is missing or ambiguous → read `docs/epics/README.md`, show the user the list with statuses, and ask which Epic to run.

## Steps

1. Resolve the Epic file: `docs/epics/<id>-<slug>.md`. Read its Tasks.
2. **Cross-Epic dependency check**: read `docs/epics/README.md`. If this Epic depends on other Epics whose status is not `done` → refuse to run; tell the user which prerequisite Epics need to ship first.
3. **Task dependency check**: verify each Task has its dependencies listed.
   - If dependencies are missing or unclear → invoke **evo_architect** (or **gf_architect** if still in greenfield phase) to add them, then resume.
   - If a Task depends on a Task from an outside Epic (full id `NNN.TXX`) whose Epic is not `done` → flag and stop.
4. **Read the dependency graph & parallelism plan from the Epic file**. Do not recompute.
   - If the section is missing or incomplete → invoke the Architect to add it, then resume. Do not guess.
5. **Wave 1**: read the first wave from the plan.
6. **Run in parallel**: for each Task in the current wave, run `04-task-feature.md` as a subroutine. Invoke task-feature runs concurrently — they share the KB but operate on independent Tasks.
7. Wait for the wave to finish.
8. If any Task failed, was blocked, or surfaced an upstream decision → pause and ask the user how to proceed (fix, skip, restructure deps, escalate to evolution).
9. Mark completed Tasks. Identify the next wave (Tasks whose deps are now satisfied). Repeat 6–8 until every Task is done.
10. **Integration check**: run the full test suite (unit + medium + e2e) against the combined result of all Tasks in the Epic. If failures emerge only when Tasks are combined → fix in scope or surface as a new Task.
11. **Review + fix loop** on the integrated change (`11-review-fix-loop.md`).
12. Update `docs/epics/README.md`: set this Epic's status to `done`.
13. **kb-curator** → mandatory **pattern-extraction pass** (see `agents/kb-curator.md` → Pattern extraction): scan reviewer findings across this Epic's Tasks, add recurring issues to `docs/kb/common-pitfalls.md`. Optional follow-on: consolidate per-Task docs and prune duplicates.
14. **Completion announcement** — must always happen, even after a partial / aborted Epic:
    - Print to the user (terminal output) a clearly visible message:
      `=== Epic <id> "<title>" completed ===`
      followed by a one-line summary (tasks done, tests green, reviewers clean).
    - Append a matching entry to `DevTeam.log`:
      `[<ISO ts>] [orchestrator] [Epic <id> completed] <title> — <summary>`
    - If the Epic ended early (aborted, blocked, partial) use `aborted` / `partial` instead of `completed` in both messages, and state the reason.

## Output
- Every Task in the Epic implemented, tested, reviewed.
- Integration tests green against the combined result.
- Optional: consolidated Epic doc in the KB.

## Done when
- All Tasks in the Epic meet their acceptance criteria.
- Integration test suite is green.
- Reviewers report no critical / high findings on the integrated change.
- Index status set to `done`.
- Completion announcement printed to the user and logged to `DevTeam.log`.

## Notes
- **Parallelism rules**:
  - Run Tasks in parallel only when their deps allow.
  - Avoid parallelizing Tasks that touch the same files heavily — merge-conflict cost outweighs speed gain. Serialize those.
  - Backend / frontend Tasks with a shared contract can run in parallel as long as the contract is locked.
- **Recovery**: if a Task fails mid-wave, finish the rest of the wave before deciding on the failure. Don't cascade-cancel.
- **Cross-Epic deps**: never resolve silently. Always surface to the user.
