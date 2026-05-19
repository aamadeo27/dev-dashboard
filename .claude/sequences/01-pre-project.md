# Sequence: Pre-project definition

Goal: lock everything down before any code is written. Output is a complete spec the rest of the sequences consume.

## When to use
- New project, no code yet.
- You want full alignment on goal, scope, design, architecture, infra, monitoring before bootstrap.

## Inputs
- Project idea, target users, constraints (if any).

## Steps

1. **gf_requirement-engineer** → Requirements doc (goal, priorities, actions, gaps closed)
2. **gf_ui-ux-designer** → UI/UX spec (screens, flows, components, color palette, gaps flagged if any)
   - If gaps flagged, loop back to step 1.
3. **gf_architect** → Knowledge Base v1 (system design, stack, patterns, contracts, conventions) + Epics/Tasks + monitoring direction
   - If gaps flagged, loop back to step 1 or 2.
4. **gf_devops-engineer** → DevOps plan + branching/PR pattern + secrets pattern (added to KB)
5. **monitor** → concrete monitoring config (queries, alerts, dashboards) + setup tasks (added to KB)
6. **scope-reviewer** → final pass on the spec for coverage gaps and silent decisions

## Output
- Requirements doc
- UI/UX spec
- Knowledge Base (system design, stack, patterns, contracts, conventions, monitoring, branching/PR, secrets)
- Epics + Tasks ready for implementation

## Done when
- Scope-reviewer reports no coverage gaps and no escalations.
- User has signed off on the spec.
