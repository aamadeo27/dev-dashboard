# How to use the sequences

Sequences are recipes. Each one names which agents run, in what order, with what inputs and exit criteria. Pick one based on what you are trying to do, then execute it step by step.

## How to invoke a sequence

Use the `/sequence` slash command:

```
/sequence <name>
```

Examples:

```
/sequence pre-project
/sequence task
/sequence bug-fix
/sequence evolution
/sequence refactor
```

Aliases accepted: `task` → `task-feature`, `bug` → `bug-fix`, `kb` → `kb-curation`, `dep` → `dep-cve-patch`, `backfill` → `test-backfill`, `review` → `review-fix-loop`, `monitoring` → `monitoring-rollout`.

What happens:
1. Claude resolves `<name>` to the right `sequences/<n>-<name>.md` file.
2. Reads it and checks the **Inputs** are present (asks you if not).
3. Executes each step in order, invoking the named agent for that step.
4. Pauses between major phases for your feedback.
5. Loops where the sequence says (e.g., review + fix loop, gap-back loops).
6. Stops at **Done when** and confirms exit criteria with you.

Run `/sequence` with no name to get the list of available sequences.

## Manual invocation

You can also run a sequence by hand without the slash command — just open the file and invoke each agent yourself in the order listed. The slash command is sugar.

## Quick map

| Situation | Sequence |
|---|---|
| Brand new project, nothing defined yet | `01-pre-project` |
| Spec is locked, time to scaffold | `02-bootstrap` |
| Dev works, need to ship to real users | `03-prod-infra` |
| Build a single feature / Task | `04-task-feature` |
| New feature on an existing app | `05-evolution` |
| Fix a reported bug | `06-bug-fix` |
| KB feels messy or stale | `07-kb-curation` |
| CVE flagged or dep upgrade needed | `08-dep-cve-patch` |
| Pay down code-quality debt | `09-refactor` |
| Legacy code with no tests | `10-test-backfill` |
| (Called from other sequences) | `11-review-fix-loop` |
| Add or upgrade monitoring later | `12-monitoring-rollout` |

## Typical project lifecycle

```
01-pre-project   →   02-bootstrap   →   03-prod-infra
                                              ↓
                  ┌───────────────────────────┴────────────────────────┐
                  ↓                                                    ↓
        04-task-feature                                         05-evolution
        (per Task in initial backlog)                           (per change request)
                  ↓                                                    ↓
                  └────────────────────  06-bug-fix  ──────────────────┘
                                              │
                          ┌───────────────────┼─────────────────────┐
                          ↓                   ↓                     ↓
                    07-kb-curation    08-dep-cve-patch        09-refactor
                          ↓                                         ↓
                    10-test-backfill                        12-monitoring-rollout
```

`11-review-fix-loop` is a **subroutine** — never run alone. Called by 02, 03, 04, 05, 06, 08, 09.

## How to pick

1. **Greenfield or existing app?**
   - Greenfield, no spec → start at `01`.
   - Existing app → skip to one of `04`–`12`.
2. **Bug or feature?**
   - Reported bug, local scope → `06`.
   - New behavior, design impact → `05` (then `04` per Task).
3. **Code change or housekeeping?**
   - Code change → `04`/`05`/`06`/`08`/`09`.
   - Housekeeping → `07`/`10`/`12`.

## Rules of engagement

- **Read inputs before invoking the first agent**. Each sequence lists its inputs — if any are missing, fix that first (often by running an earlier sequence).
- **Respect agent lanes**. Agents will surface decisions outside their lane. When that happens, route the question to the right agent (or user), then resume.
- **Loop, don't skip**. If a sequence step flags gaps, loop back. Skipping leads to scope creep and rework.
- **One sequence at a time per scope**. Don't mix a refactor into a feature, or a CVE patch into a bug fix. Cleaner diffs, cleaner reviews.

## Agent locations (reference)

```
.claude/agents/
├── coder.md                 ─ implements Tasks
├── tester.md                ─ medium + e2e tests
├── monitor.md               ─ monitoring config + alerts
├── kb-curator.md            ─ KB hygiene
├── greenfield/              ─ project-start agents (name: gf_*)
├── evolution/               ─ change-request agents (name: evo_*)
└── reviewers/               ─ perf / security / scope / code-quality
```

## When to write a new sequence

Add one when:
- You repeat the same multi-agent flow 3+ times.
- You need a variant of an existing sequence with materially different steps.
- A new agent enters the team and changes how work flows.

Keep new sequences in the same format: when-to-use, inputs, steps, output, done-when.
