# Sequence: Evolution / change request

Goal: take a change request against an existing app from idea to implemented Task(s).

## When to use
- App is already running (or at least past bootstrap).
- A new feature, change, or non-trivial enhancement is requested.
- Use `06-bug-fix.md` instead for bugs.

## Inputs
- Change request (informal description from user or stakeholder).
- Existing Requirements, UI/UX, Knowledge Base, code.

## Steps

1. **evo_requirement-engineer** → Change Request doc (motivation, affected actions, regression boundaries, migration notes)
2. **evo_ui-ux-designer** → UI changes that fit existing design system; flags consistency deviations
   - Loop back if gaps.
3. **evo_architect** → KB update (diff), new contracts/migrations, Tasks (one Epic if >2 tasks), impact & risk summary
   - Loop back if gaps.
4. **evo_devops-engineer** → DevOps Change Plan (infra delta, env config changes, rollout, feature flags)
5. **monitor** → updated queries/alerts for new key flows
6. **scope-reviewer** → final pass on the change spec before coding starts
7. For each Task → run `04-task-feature.md`

## Output
- Change Request doc
- Updated KB
- Tasks implemented, tested, reviewed
- Rollout plan in place

## Done when
- All Tasks done per `04-task-feature.md`.
- Regression tests pass.
- Change deployed to prod per DevOps Change Plan.
