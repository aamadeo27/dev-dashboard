# Future agents

Agents not built yet. Add when needed.

## Accessibility reviewer
Reviews UI changes for a11y: semantic HTML, ARIA, keyboard nav, focus order, contrast, screen-reader labels. Single lane — no perf/security/scope/code-quality.

## Release coordinator
Decides when a change is "done done" and deployable. Verifies: reviewers signed off, tests green, monitoring updated, KB updated, deploy plan ready. Owns the go/no-go.

## Incident / oncall agent
Post-prod role. Reacts to alerts, triages incidents, runs initial diagnosis, drafts postmortems. Hooks into Monitor outputs.

## Change triage
Evolution-mode role. Sits before `evo_requirement-engineer`. Takes raw change requests / bug reports, classifies (bug / feature / chore), prioritizes against current work, decides whether to spin up the evolution flow.
