# Sequence: Test backfill

Goal: add missing test coverage to existing code, retroactively.

## When to use
- Inherited / legacy code with low coverage.
- Coverage report shows a gap on critical paths.
- Before a risky refactor in a poorly tested area.

## Inputs
- Target area (files / module / feature) to backfill.
- Reason for coverage gap (legacy, deadline pressure, etc.).

## Steps

1. **tester** → reads existing code + Task docs (if any) + Requirements; lists scenarios that should exist as tests
2. **tester** → adds tests at the right levels:
   - Unit tests for pure logic (if missing — note: normally Coder's job, but acceptable in backfill)
   - Medium tests for component wiring and DB interaction
   - E2E tests for user-visible flows
3. **tester** → runs the suite, reports current pass rate and coverage delta
4. If tests reveal actual bugs (not just gaps) → fork to `06-bug-fix.md` for each
5. **scope-reviewer** → confirms backfill did not change behavior or add features

## Output
- New tests across appropriate levels
- Coverage delta report
- List of any bugs surfaced

## Done when
- Coverage on target area meets the agreed bar.
- All new tests green; no existing test broken.

## Notes
- Backfill is not a license to refactor. If code is so tangled it can't be tested, raise it and run `09-refactor.md` first.
