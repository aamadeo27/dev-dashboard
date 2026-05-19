# Sequence: Epic execution

Goal: drive every Task in an Epic to completion, running independent Tasks in parallel.

## When to use
- Architect has produced an `Epics.md` (or equivalent) with one or more Epics, each containing Tasks.
- You're ready to implement one full Epic.
- For a single ad-hoc Task, use `04-task-feature.md` directly. For a brand-new change request, use `05-evolution.md`.

## Inputs
- The target Epic (id or title).
- `Epics.md` with Tasks per Epic.
- Knowledge Base (contracts, conventions, patterns).

## Steps

1. Read the target Epic and list its Tasks.
2. **Dependency check**: verify each Task has its dependencies listed.
   - If dependencies are missing or unclear → invoke **evo_architect** (or **gf_architect** if still in greenfield phase) to add them, then resume.
   - If a Task depends on something outside this Epic → flag it and ask the user whether to proceed, defer, or restructure.
3. Build the dependency graph for the Epic's Tasks.
4. **Wave 1**: identify Tasks with no unresolved deps.
5. **Run in parallel**: for each Task in the current wave, run `04-task-feature.md` as a subroutine. Invoke task-feature runs concurrently — they share the KB but operate on independent Tasks.
6. Wait for the wave to finish.
7. If any Task failed, was blocked, or surfaced an upstream decision → pause and ask the user how to proceed (fix, skip, restructure deps, escalate to evolution).
8. Mark completed Tasks. Identify the next wave (Tasks whose deps are now satisfied). Repeat 5–7 until every Task is done.
9. **Integration check**: run the full test suite (unit + medium + e2e) against the combined result of all Tasks in the Epic. If failures emerge only when Tasks are combined → fix in scope or surface as a new Task.
10. **Review + fix loop** on the integrated change (`11-review-fix-loop.md`).
11. **kb-curator** → optional pass to consolidate per-Task docs into a single Epic-level doc and prune duplicates.

## Output
- Every Task in the Epic implemented, tested, reviewed.
- Integration tests green against the combined result.
- Optional: consolidated Epic doc in the KB.

## Done when
- All Tasks in the Epic meet their acceptance criteria.
- Integration test suite is green.
- Reviewers report no critical / high findings on the integrated change.

## Notes
- **Parallelism rules**:
  - Run Tasks in parallel only when their deps allow.
  - Avoid parallelizing Tasks that touch the same files heavily — merge-conflict cost outweighs speed gain. Serialize those.
  - Backend / frontend Tasks with a shared contract can run in parallel as long as the contract is locked.
- **Recovery**: if a Task fails mid-wave, finish the rest of the wave before deciding on the failure. Don't cascade-cancel.
- **Cross-Epic deps**: never resolve silently. Always surface to the user.
