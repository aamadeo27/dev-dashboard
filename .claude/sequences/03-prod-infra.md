# Sequence: Prod infra setup

Goal: provision the production environment. Separate from bootstrap because risk and access are different.

## When to use
- Dev environment exists and is stable.
- Ready to deploy somewhere real.

## Inputs
- DevOps plan from pre-project sequence (or current state thereof).
- Knowledge Base (secrets pattern, monitoring direction).

## Steps

1. **gf_devops-engineer** → confirms prod plan: hosting, networking, DB, secrets manager, backups, scaling targets
2. **gf_devops-engineer** → provisions prod infra (IaC if defined, else documented manual steps)
3. **coder** → finalizes prod-ready CD pipeline (deploy stage, rollout strategy, rollback)
4. **monitor** → wires prod monitoring (real tool, real alerts, real dashboards, on-call channel if defined)
5. **security-reviewer** → checks secrets handling, TLS, headers, exposed surfaces
6. **performance-reviewer** → sanity check on cold-start, instance size, DB capacity vs expected load
7. **scope-reviewer** → checks nothing was added beyond Requirements (no extra envs, no over-provisioning)
8. **Review + fix loop** (see `11-review-fix-loop.md`) only for findings from steps 5–7

## Output
- Prod environment provisioned and reachable
- CD pipeline deploys to prod on protected trigger
- Monitoring live with alerts wired
- Rollback procedure documented and tested

## Done when
- A trivial change can be deployed to prod and rolled back without manual intervention.
- Alerts fire on synthetic failure.

## Notes
- Run this BEFORE the first real feature ships to prod, not after.
