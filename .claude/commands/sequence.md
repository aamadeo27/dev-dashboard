---
description: Run a process sequence end-to-end. Usage `/sequence <name>` (e.g. `/sequence pre-project`).
argument-hint: <sequence-name>
---

You are the orchestrator for a process sequence.

## What to do

1. Take the argument: `$ARGUMENTS`. If the user appended `user-decides-tech` (or `udt`) as an extra token, set the **tech-decision mode** to `user`. Otherwise default to `autonomous` (Architect decides technical issues).
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
   - `epic-execution` / `epic` → `.claude/sequences/13-epic-execution.md`
3. If no argument or no match: list available sequences and stop.
4. Read the matching sequence file in full.
5. Verify the **Inputs** listed in the sequence are present. If any are missing, ask the user before starting.
6. Execute the steps in order. For each step that names an agent, invoke that agent via the Task tool with the inputs the sequence specifies. When invoking the Architect (gf_architect or evo_architect), pass the tech-decision mode in the prompt (`tech-decision mode: autonomous` or `tech-decision mode: user`). Report each agent's output to the user before moving on.
7. Loop where the sequence says to loop (e.g., review + fix loop, gap-back loops).
8. Stop at the **Done when** conditions. Confirm with the user that exit criteria are met.

## Rules

- Do not skip steps.
- Do not invoke an agent outside its lane — if a sequence step needs an agent not listed, ask.
- Pause between major phases for user feedback unless the user said "run autonomously".
- Surface every agent's flagged gaps or escalations to the user — do not silently resolve them.
- If a sequence calls another sequence (e.g., `04-task-feature` → `11-review-fix-loop`), run it as a subroutine in place.

## Logging

Maintain `DevTeam.log` at the project root with the same format the agents use:

```
[<ISO-8601 UTC timestamp>] [<agent-name>] [<short title>] <one-line description>
```

Log as the orchestrator (`<agent-name>` = `orchestrator`) at these moments:
- Sequence start: `[Sequence started] <sequence-name> mode=<autonomous|user>`
- Step transition: `[Step N -> N+1] <step name> -> <next step name>`
- Loop iteration: `[Loop] <which loop> iteration <n>`
- Sequence end: `[Sequence done] <sequence-name>` (or `[Sequence aborted] <reason>`)

Agent invocations log themselves per their own Logging rule — do not double-log on their behalf.

## Usage checkpoints

Before invoking any agent, emit a checkpoint line so external tooling can compute token cost between checkpoints:

```
[<ts>] [orchestrator] [Usage checkpoint] phase=<phase> sequence=<seq> task=<id?> iteration=<n?> model=<model-being-invoked>
```

Required checkpoints:
- **Sequence start**: `phase=sequence-start sequence=<name>`
- **Wave start** (epic-execution): `phase=wave-start wave=<n> tasks=[<list>]`
- **Before each agent invocation**: `phase=agent-pre agent=<name> task=<id?> iteration=<n?> model=<model>`
- **After each agent invocation**: `phase=agent-post agent=<name> task=<id?> iteration=<n?>`
- **Review pass start / end**: `phase=review-start|review-end task=<id> iteration=<n>`
- **Fix pass start / end**: `phase=fix-start|fix-end task=<id> iteration=<n>`
- **Sequence end**: `phase=sequence-end sequence=<name>`

A separate `Stop` / `SubagentStop` hook (see project docs) reads transcript usage and emits matching `[Usage tokens] in=N out=N cache_read=N cache_create=N` lines. Cost per phase = sum of `[Usage tokens]` lines between two checkpoints.

## Model routing (Haiku / Sonnet / Opus)

Each agent has a default model in its frontmatter. Before every invocation, decide if Haiku is safe to use. If yes → override to Haiku. If unsure → keep the default.

### Haiku triggers — use Haiku only when ALL conditions hold

Run through this checklist before each invocation. If every condition is `yes`, route to Haiku. Any `no` → fall back to the agent's default model.

**Trigger H-FIX (coder fix pass)** — use Haiku when:
1. Mode is `fix-pass` (not Fresh).
2. The findings file lists **≤5 fixes** for this iteration.
3. No finding is severity **CRIT or HIGH** under the **security** lane.
4. No file touched by the fixes lives under security-sensitive paths (auth, crypto, secrets, validation, session, token).
5. No fix changes a public API contract, data model, or env-var shape.

**Trigger H-RR (reviewer re-review, non-security)** — use Haiku when:
1. This is a **re-review** (not the first pass on a Task).
2. The reviewer is **performance**, **scope**, or **code-quality** (never security).
3. The fix diff being reviewed is **< 200 lines changed**.
4. No file in the diff lives under security-sensitive paths.

**Trigger H-KBP (kb-curator pattern-extraction)** — always use Haiku for the post-Epic pattern-extraction pass. Mechanical aggregation, no judgment that justifies Sonnet.

**Trigger H-SCOPE (scope-reviewer on small diff)** — use Haiku when:
1. Reviewer is **scope-reviewer**.
2. Diff being reviewed is **< 100 lines changed**.
3. No new files added; no files deleted.

### Never Haiku

Always keep the default model for:
- Architect (any), requirement-engineer (any), monitor, security-reviewer at any pass, tester at any pass, devops-engineer at any pass, ui-ux-designer at any pass.
- Coder **Fresh** mode.
- Reviewer **first pass** on a Fresh Task.
- kb-curator full curation pass (not the pattern-extraction sub-pass).

### Verbose Haiku logging

Whenever you route to Haiku, emit a dedicated log line **immediately before** the Task invocation, in addition to the normal `[Usage checkpoint]` line:

```
[<ts>] [orchestrator] [Haiku route] agent=<agent-name> task=<task-id?> iteration=<n?> trigger=<H-FIX|H-RR|H-KBP|H-SCOPE> reason="<one-line why>"
```

Include the **trigger id** so we can audit which rule fired. Examples:

```
[2026-05-24T10:14:02Z] [orchestrator] [Haiku route] agent=coder task=T2.5 iteration=3 trigger=H-FIX reason="4 mechanical fixes, no sec, ProjectCard.tsx only"
[2026-05-24T10:18:11Z] [orchestrator] [Haiku route] agent=code-quality-reviewer task=T2.5 iteration=3 trigger=H-RR reason="re-review of 38-line fix diff"
[2026-05-24T11:02:55Z] [orchestrator] [Haiku route] agent=kb-curator trigger=H-KBP reason="post-Epic pattern extraction"
```

If you considered Haiku but rejected it, log that too — single line, so we can later tune the rules:

```
[<ts>] [orchestrator] [Haiku skipped] agent=<name> task=<id?> reason="<which condition failed>"
```

### Sequence-end summary

At sequence end, before the `[Sequence done]` line, emit one summary line:

```
[<ts>] [orchestrator] [Model usage summary] sequence=<name> haiku=<count> sonnet=<count> opus=<count> haiku-skipped=<count>
```

Counts agent invocations only (not the orchestrator's own turns).
