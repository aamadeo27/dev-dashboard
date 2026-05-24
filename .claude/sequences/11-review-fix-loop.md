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
1. Run in parallel, each invoked with the diff path + changed-files list as inputs (no broad search allowed):
   - **performance-reviewer**
   - **security-reviewer**
   - **scope-reviewer**
   - **code-quality-reviewer**
2. Collect findings, grouped by severity. Persist them to `docs/tasks/<task-id>-findings.md` (append for each iteration; never overwrite history).
3. If no critical or high findings → done.
4. **Re-review planning** (orchestrator) — **before** dispatching the coder, decide which reviewers will need to run after the fix pass. Default rule:
   - Re-run every reviewer that had a critical or high finding the coder must address.
   - Also re-run any reviewer whose lane is touched by the planned fix area (e.g., if fixes touch async code, perf reviewer re-runs even if it had no findings).
   - Skip reviewers with zero findings whose lane the fix doesn't touch.
   - Record the planned re-review set in the findings file as `Re-review plan: [perf, quality]` (etc.).
5. **coder** addresses findings — invoke in **Fix pass** mode (see `agents/coder.md` → Invocation modes). The invocation prompt must include:
   - `mode: fix-pass`
   - `task-id: <task-id>`
   - The path to `docs/tasks/<task-id>-findings.md`
   - The path to `docs/tasks/<task-id>.md`
   Coder reads both files at start, applies only the listed fixes, appends a fix summary to the Task doc.
6. Run **only the reviewers from the re-review plan**, in parallel. Do not run the others.
7. Repeat from step 2 until clean.

## Exit conditions
- All four reviewers report no critical and no high findings.
- Medium / low findings are noted but do not block merge unless user says otherwise.

## Notes
- Run reviewers in parallel; they don't depend on each other.
- If a reviewer keeps re-finding the same issue across loops, escalate — the fix may belong to a different agent.
- Sequence variants (bug fix, dep-cve-patch) may use a reduced reviewer set; defer to the calling sequence.
