# Sequence: Monitoring rollout

Goal: add or upgrade monitoring on an existing app. Use when monitoring wasn't set up at bootstrap or needs a tier upgrade.

## When to use
- App in prod without proper monitoring.
- Requirements upgraded monitoring level (e.g., SLO introduced, on-call needed).
- Adding observability to a new subsystem after the fact.

## Inputs
- Current monitoring state (or absence).
- Requirements / SLOs driving the upgrade.

## Steps

1. **evo_architect** (or **gf_architect** if no prior monitoring exists) → set monitoring direction: level, tool family, must-have signals
2. **monitor** → concrete config:
   - Queries / metrics / log structure
   - Alert rules (condition, severity, channel)
   - Dashboards
   - Instrumentation tasks for Coder
3. **evo_devops-engineer** (or **gf_devops-engineer**) → infra for monitoring stack if self-hosted; secrets/credentials for external tools
4. For each instrumentation task → run `04-task-feature.md` (or a slimmer variant if pure config)
5. **monitor** → smoke-test alerts (synthetic failure should page; recovery should clear)
6. **scope-reviewer** → confirms only Requirement-driven monitoring was added (no gold-plating)

## Output
- Live monitoring at the chosen level
- Alerts wired and tested
- Dashboards available to the team
- KB updated with monitoring config

## Done when
- Synthetic failure triggers the expected alert end-to-end.
- All key flows have signals.
- No critical scope-reviewer findings.
