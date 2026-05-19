# Sequence: Bug fix (fast lane)

Goal: fix a bug with minimum overhead, without skipping regression safety.

## When to use
- Reported bug, clear repro.
- Fix is local to one component or a small set.
- If the bug reveals a missing feature, design flaw, or cross-component impact, use `05-evolution.md` instead.

## Inputs
- Bug report: repro steps, expected vs actual, severity, affected env.

## Steps

1. **coder** → reproduces the bug locally, identifies root cause, writes a failing unit test that captures the bug
2. **coder** → implements the fix; failing test now passes
3. **coder** → updates the relevant Task doc (or creates a small one) noting: root cause, fix, regression risk
4. **tester** → adds a medium and/or e2e regression test if the bug crossed component boundaries
5. **Review + fix loop** — reduced: scope-reviewer + the reviewer matching the bug's domain (perf bug → performance-reviewer; security bug → security-reviewer; else code-quality-reviewer)

## Output
- Bug fixed, regression test in place, root cause documented

## Done when
- Failing test now passes; no other tests regress.
- Relevant reviewers report no blockers.

## Notes
- If during fix you discover the bug is symptom of a wider issue, **stop** and escalate to the evolution sequence.
