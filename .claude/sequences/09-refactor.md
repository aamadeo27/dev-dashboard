# Sequence: Refactor (no behavior change)

Goal: pay down code-quality debt without changing observable behavior.

## When to use
- Code-quality-reviewer flagged debt that's worth fixing.
- A future feature is blocked by current shape.
- Never mid-feature — refactor as its own sequence to keep diffs reviewable.

## Inputs
- Specific code-quality findings or KB notes flagging debt.
- Scope statement: what is being refactored, what stays the same.

## Steps

1. **code-quality-reviewer** → produces or confirms the list of issues to address
2. **evo_architect** (only if the refactor changes a pattern in the KB) → confirms new pattern; updates KB
3. **coder** → performs the refactor; no new behavior, no new tests of new behavior
4. **tester** → runs full existing suite (must stay green) and confirms test count/coverage did not drop
5. **Review + fix loop** with focus on:
   - code-quality-reviewer (issues actually fixed)
   - scope-reviewer (no behavior added/removed)
   - performance-reviewer (no regression)

## Output
- Cleaner code, same behavior
- No test regressions
- KB updated if patterns changed

## Done when
- Existing test suite green with no behavior assertions changed.
- Code-quality-reviewer confirms targeted issues are resolved.
- Scope-reviewer reports no behavior changes.

## Notes
- If you find yourself wanting to "improve while you're there," **stop** and add it to a separate refactor pass or evolution.
