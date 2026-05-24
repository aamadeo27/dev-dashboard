---
description: Run a process sequence end-to-end. Usage `/sequence <name>` (e.g. `/sequence pre-project`).
argument-hint: <sequence-name>
---

You are the orchestrator for a process sequence.

## What to do

1. Take the argument: `$ARGUMENTS`. If the user appended `user-decides-tech` (or `udt`) as an extra token, set the **tech-decision mode** to `user`. Otherwise default to `autonomous` (Architect decides technical issues).
   - **Task-id argument**: for sequences that operate on a specific Task (`task-feature`) or Epic (`epic-execution`), an extra token may be the id:
     - **Task id** examples: `T2.5`, `2.5`, `002.T05`. Resolve by scanning `docs/epics/*.md` for a Task matching the id (any format), then pass the resolved Task to the sequence.
     - **Epic id** examples: `2`, `002`, `002-auth`. Resolve via `docs/epics/README.md`.
   - If a sequence needs an id but none was given → read the index, list options, and ask.
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

## Code access — keep reads scoped

Subagents do not share live context; each one re-reads from disk. Sloppy prompts cause broad Globs and 4× repeated reads. Follow these rules:

1. **Name the files.** Always pass an explicit changed-files list (or path list) to agents that operate on code (reviewers, tester, coder fix passes). Do not let them discover the scope via broad search.
2. **Pre-stage a diff file.** Before any review pass that follows a code change, dump the diff to `docs/tasks/<task-id>-diff-<iter>.patch` (use `git diff` against the base commit). Pass that path to every reviewer. They Read the patch first; the patch is much smaller than the full files.
3. **Reference the KB index, never dump KB content.** Orchestrator and agents pass paths into KB sub-docs; never inline the index or sub-doc content.
4. **No "explore the repo" prompts.** Never instruct an agent to "look around" or "find relevant files." If you can't name a path or path list, pause and resolve it yourself before invoking.

## Single-Task branch lifecycle (no worktree)

For `/sequence task <id>` (and other sequences that work on one Task at a time: `bug-fix`, `dep-cve-patch`, `refactor`), the orchestrator creates a branch in the **main repo** — no worktree needed because there's no parallelism. Apply the same merge + conflict-resolution rules as the worktree flow, minus the worktree create/remove steps.

1. **Before invoking the coder**: create a branch off the current integration/default branch per the DevOps KB convention:
   ```
   git checkout -b <type>/<task-id>-<slug>
   ```
2. **Invoke agents** in the main repo (no `worktree=` input; agents use default `DevTeam.log` at project root).
3. **After review-fix loop is clean**:
   - Switch back to the integration/default branch.
   - Merge the Task branch (`git merge --no-ff <type>/<task-id>-<slug>`).
   - Apply the **Conflict resolution** rules below (same as worktree flow).
4. **Optional cleanup**: delete the merged branch (`git branch -d ...`) unless the DevOps plan keeps Task branches.

If the working tree is dirty when this sequence starts → refuse to proceed; ask the user to commit or stash.

## Worktree management (parallel Tasks)

When a sequence runs Tasks in parallel within a wave (currently only `13-epic-execution`), each Task runs in its **own git worktree** on its own branch. Sequential Tasks (single-Task waves) run in the main repo directly.

### Lifecycle per wave

1. **Before launching the wave** (from main repo, on the `integration` branch — or default branch if none):
   - For each Task in the wave:
     ```
     git worktree add ../<repo-name>-<task-id> -b <type>/<task-id>-<slug> <integration-branch>
     ```
     Use the branching convention from the DevOps KB (`feat/`, `fix/`, etc.).
   - Record the worktree path → Task id mapping for the wave.
2. **Invoke each Task's agents** with the worktree path as input. Every agent prompt must include:
   - `worktree=<absolute-path>`
   - `log_file=<worktree>/DevTeam.<task-id>.log` (overrides each agent's default `DevTeam.log`; agents log there for the duration of the Task)
   Pass absolute paths for all other inputs (diff, findings, task doc) so agents don't need to resolve worktree-relative paths.
3. **After the wave finishes**, in the main repo:
   - Merge each Task branch sequentially into the integration branch:
     ```
     git merge --no-ff <type>/<task-id>-<slug>
     ```
   - **Resolve conflicts** per the Conflict resolution rules below. You (the orchestrator) own this — do not punt to the user except as a last resort.
   - **Concat per-Task logs**: each worktree wrote to `DevTeam.<task-id>.log`. After merge, append all per-Task log entries to the main `DevTeam.log`, sorted by timestamp. Then delete the per-Task fragments (they're committed in the merge, can be removed via a follow-up commit if you don't want them in history).

### Conflict resolution

When `git merge` fails with conflicts, the orchestrator handles it. Classify each conflict and act:

1. **Trivial textual** (formatter output, import ordering, line endings, generated files, `package-lock.json` / `pnpm-lock.yaml`):
   - Resolve directly. Strategy: keep both Tasks' intent (union for imports, lockfile regenerate, formatter re-run on the merged file).
   - Stage, continue the merge.
2. **Same-region edits where one Task's change is a strict superset of the other** (rare but happens):
   - Keep the superset. Stage, continue.
3. **Semantic conflict** — two Tasks made incompatible changes to the same logic, function signature, or contract:
   - Do **not** guess. Dispatch a **Coder fix pass** scoped to the conflict, with both Task docs + the merge-conflict markers as input. Coder reconciles intent.
   - This signals an Architect failure (parallelism plan should have prevented it). Log it and feed it back: `[Architect feedback] Tasks <a>+<b> conflicted; parallelism plan needs revision`. kb-curator picks this up in the post-Epic pass.
4. **Test conflict** — both Tasks edited the same test file:
   - Usually trivial union. If tests assert contradictory behaviors → semantic conflict, follow rule 3.
5. **Unresolvable / unsafe** (you're not sure which side is right after one Coder reconciliation pass):
   - **Then** pause and ask the user. Last resort.

Always log every conflict and its resolution:

```
[<ts>] [orchestrator] [Merge conflict] task=<id> files=[<paths>] kind=<trivial|superset|semantic|test|unresolvable> resolution=<auto|coder|user>
```

Never use `git merge -X ours` or `-X theirs` blindly — those drop changes silently.
4. **Remove worktrees**:
   ```
   git worktree remove ../<repo-name>-<task-id>
   ```

### Rules

- Never run agents from two parallel Tasks against the same worktree.
- The KB (`docs/kb/`) lives in the main repo and is read-only during a wave. If a Task needs a KB change → surface as escalation; do not mutate KB inside a worktree.
- Per-Task files (`docs/tasks/<task-id>.md`, `<task-id>-findings.md`, `<task-id>-diff-<iter>.patch`, `DevTeam.<task-id>.log`) live **inside** the worktree and travel with the merge.
- Merge conflicts on `DevTeam.log` are avoided by using per-Task log fragments and only writing the consolidated `DevTeam.log` from the main repo after merge.
- For single-Task waves, skip the worktree dance entirely — work directly in the main repo on a branch per the DevOps pattern.

## Subagent prompt construction

How you build the prompt for each Task tool invocation directly affects token cost and cache hit rate. Follow these three rules:

1. **Pass file paths, not content.** Do not inline the Knowledge Base, Task docs, findings, UI/UX specs, or any other persistent doc into the prompt. Pass the path; the agent will Read what it needs. Inlined content burns input tokens every call and defeats prompt caching.
   - Bad: `Here is the KB: <2KB blob>...`
   - Good: `Read docs/kb/index.md and the patterns/contracts entries relevant to your Task.`

2. **Do not re-state the agent's own rules.** Each agent's `.md` body is already its system prompt — it knows its lane, output format, and process. Your user prompt should only carry the **inputs** for this invocation (Task id, paths, mode, tech-decision mode, etc.) — not a restatement of what the agent does.
   - Bad: `You are the Coder. Implement only the assigned Task. Stay in scope. Read the KB...`
   - Good: `Task: T2.5. Mode: fix-pass. Findings: docs/tasks/T2.5-findings.md. Task doc: docs/tasks/T2.5.md.`

3. **Stable prefix, variable tail.** Build every Task prompt in this fixed order so the Anthropic prompt cache hits across invocations:
   1. **Role + mode line** (e.g., `task=T2.5 mode=fix-pass iteration=3`)
   2. **Static input paths block** (KB pointer, Task doc path, findings path, UI/UX spec path) — same shape every call
   3. **Sequence context** (sequence name, current phase) — same shape every call
   4. **Variable delta** (this iteration's specific instruction, e.g., "address findings F-3 and F-7") — at the end

   Putting the variable part last keeps the prefix stable, which lets the cache match across iterations and across tasks within the same Epic.

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
