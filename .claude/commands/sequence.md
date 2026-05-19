---
description: Run a process sequence end-to-end. Usage `/sequence <name>` (e.g. `/sequence pre-project`).
argument-hint: <sequence-name>
---

You are the orchestrator for a process sequence.

## What to do

1. Take the argument: `$ARGUMENTS`.
2. Find the matching file in `.claude/sequences/`. Match by suffix, ignoring leading digits and the `.md` extension.
   - `pre-project`        → `.claude/sequences/01-pre-project.md`
   - `bootstrap`          → `.claude/sequences/02-bootstrap.md`
   - `prod-infra`         → `.claude/sequences/03-prod-infra.md`
   - `task-feature` / `task` → `.claude/sequences/04-task-feature.md`
   - `evolution`          → `.claude/sequences/05-evolution.md`
   - `bug-fix` / `bug`    → `.claude/sequences/06-bug-fix.md`
   - `kb-curation` / `kb` → `.claude/sequences/07-kb-curation.md`
   - `dep-cve-patch` / `dep` → `.claude/sequences/08-dep-cve-patch.md`
   - `refactor`           → `.claude/sequences/09-refactor.md`
   - `test-backfill` / `backfill` → `.claude/sequences/10-test-backfill.md`
   - `review-fix-loop` / `review` → `.claude/sequences/11-review-fix-loop.md`
   - `monitoring-rollout` / `monitoring` → `.claude/sequences/12-monitoring-rollout.md`
3. If no argument or no match: list available sequences and stop.
4. Read the matching sequence file in full.
5. Verify the **Inputs** listed in the sequence are present. If any are missing, ask the user before starting.
6. Execute the steps in order. For each step that names an agent, invoke that agent via the Task tool with the inputs the sequence specifies. Report each agent's output to the user before moving on.
7. Loop where the sequence says to loop (e.g., review + fix loop, gap-back loops).
8. Stop at the **Done when** conditions. Confirm with the user that exit criteria are met.

## Rules

- Do not skip steps.
- Do not invoke an agent outside its lane — if a sequence step needs an agent not listed, ask.
- Pause between major phases for user feedback unless the user said "run autonomously".
- Surface every agent's flagged gaps or escalations to the user — do not silently resolve them.
- If a sequence calls another sequence (e.g., `04-task-feature` → `11-review-fix-loop`), run it as a subroutine in place.
