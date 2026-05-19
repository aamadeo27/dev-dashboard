# Sequence: Task / feature implementation

Goal: implement one Task from the backlog end-to-end.

## When to use
- A Task exists in the backlog (from pre-project or evolution sequence).
- Bootstrap is done, prod infra ideally also done.

## Inputs
- The Task (acceptance criteria, contracts, dependencies)
- Knowledge Base
- UI/UX spec (for frontend tasks)

## Steps

1. **coder** → implements the Task, writes unit tests, writes Task doc (`docs/tasks/<task-id>.md`)
   - Coder stops and surfaces any decision outside their lane.
2. **tester** → expands Task doc with Test Scenarios, writes medium + e2e tests, runs the suite
3. **Review + fix loop** (see `11-review-fix-loop.md`)
4. **monitor** → if Task introduces a new key flow, add metrics/alerts per Architect's direction

## Output
- Task implemented, unit + medium + e2e tests green
- Task doc with test scenarios in KB
- Reviewers report no blocking findings

## Done when
- All four reviewers (perf, security, scope, code-quality) report no critical/high findings.
- All test levels pass.
- Task acceptance criteria met.
