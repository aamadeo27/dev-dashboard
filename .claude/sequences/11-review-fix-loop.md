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
2. Collect findings, grouped by severity.
3. If no critical or high findings → done.
4. Otherwise → **coder** addresses findings:
   - Stays inside the original Task scope.
   - Surfaces any finding that would require an upstream decision (Architect / DevOps / UI-UX).
5. Re-run only the reviewers that had findings.
6. Repeat from step 2 until clean.

## Exit conditions
- All four reviewers report no critical and no high findings.
- Medium / low findings are noted but do not block merge unless user says otherwise.

## Notes
- Run reviewers in parallel; they don't depend on each other.
- If a reviewer keeps re-finding the same issue across loops, escalate — the fix may belong to a different agent.
- Sequence variants (bug fix, dep-cve-patch) may use a reduced reviewer set; defer to the calling sequence.
