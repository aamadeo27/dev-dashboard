# Sequence: Review + fix loop (subroutine)

Goal: after any code change, run all relevant reviewers in parallel and fix findings until clean.

This is a subroutine called by other sequences, not run standalone.

## When to use
- Called by: bootstrap, task/feature, evolution, bug-fix, dep-cve-patch, refactor.

## Inputs
- The code change to review (Task scope, bug fix, etc.).

## Steps

0. **Stage the diff** (orchestrator, before any reviewer runs):
   - Verify the working tree is clean — the coder (and tester, if it ran) must have committed before the review starts. If dirty, refuse to proceed; ask the responsible agent to commit.
   - **Base commit** for iteration N:
     - **Single-Task mode (main repo, no worktree)**:
       - Iteration 1: parent of the coder's Fresh commit (the commit immediately before this Task started).
       - Iteration N ≥ 2: the previous fix-pass commit.
     - **Parallel mode (Task in its own worktree)**:
       - Iteration 1: the `integration` branch tip at the moment the worktree was created (the worktree's base). Diff = `integration..HEAD`.
       - Iteration N ≥ 2: the previous fix-pass commit on this branch.
   - Generate `docs/tasks/<task-id>-diff-<iter>.patch` via `git diff <base>..HEAD`.
   - Build a **changed-files list** (one path per line). Dump to `docs/tasks/<task-id>-changed-<iter>.txt` or pass inline if short.
   - Both go into every reviewer invocation.
1. Run with the diff path + changed-files list as inputs (no broad search allowed):
   - **basic-reviewer** (single agent — runs scope pass then quality pass, returns two-section findings)

   Note: `performance-reviewer` and `security-reviewer` are **deferred to Epic-end** and do not run per-Task. See `13-epic-execution.md`.
2. Collect findings, grouped by severity. Persist them to `docs/tasks/<task-id>-findings.md` (append for each iteration; never overwrite history).
3. If no critical or high findings → done.
4. **Re-review planning** (orchestrator) — before dispatching the coder, decide whether the basic-reviewer needs to re-run after the fix pass. With only one reviewer per Task, the rule is simple:
   - If the previous review had any critical or high finding (scope or quality) → re-run after the fix.
   - If all findings were medium/low and the coder is only addressing those → re-run only if the fix touches the same lane as the findings; otherwise skip re-review and exit.
   - Record the decision in the findings file as `Re-review plan: [basic-reviewer]` or `[skip]`.
5. **coder** addresses findings — invoke in **Fix pass** mode (see `agents/coder.md` → Invocation modes). The invocation prompt must include:
   - `mode: fix-pass`
   - `task-id: <task-id>`
   - The path to `docs/tasks/<task-id>-findings.md`
   - The path to `docs/tasks/<task-id>.md`
   Coder reads both files at start, applies only the listed fixes, appends a fix summary to the Task doc.
6. Re-run the basic-reviewer if the plan says so. Otherwise skip.
7. Repeat from step 2 until clean.

## Exit conditions
- basic-reviewer reports no critical and no high findings (scope) and no high findings (quality).
- Medium / low findings are noted but do not block merge unless user says otherwise.
- Security and performance reviews happen at Epic-end, not here.

## Notes
- Run reviewers in parallel; they don't depend on each other.
- If a reviewer keeps re-finding the same issue across loops, escalate — the fix may belong to a different agent.
- Sequence variants (bug fix, dep-cve-patch) may use a reduced reviewer set; defer to the calling sequence.
