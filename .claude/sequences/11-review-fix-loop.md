# Sequence: Review + fix loop (subroutine)

Goal: after any code change, run all relevant reviewers in parallel and fix findings until clean.

This is a subroutine called by other sequences, not run standalone.

## When to use
- Called by: bootstrap, task/feature, evolution, bug-fix, dep-cve-patch, refactor.

## Inputs
- The code change to review (Task scope, bug fix, etc.).

## Steps

1. Run in parallel:
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
