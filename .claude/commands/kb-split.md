---
description: Split a monolithic Knowledge Base file into the standard docs/kb/ sub-doc structure.
argument-hint: [path-to-monolith]
---

You are splitting a monolithic Knowledge Base into the structure expected by every agent. **No content may be lost.** Backup first, then split.

## Inputs

- Optional argument: `$ARGUMENTS` — path to the monolithic KB file. If empty, search the project for likely candidates in this order:
  1. `docs/kb.md`
  2. `docs/knowledge-base.md`
  3. `docs/KB.md`
  4. `kb.md` at project root
  5. Any single `.md` file under `docs/` whose top-level headings include "System design", "Tech stack", "Patterns", "Contracts", "Conventions".
  If multiple candidates, list them and ask the user which.
  If `docs/kb/` already exists and contains `README.md` + sub-docs → **stop**; the KB is already split.

## Target structure

```
docs/kb/
├── README.md           short index — one line per entry, no inline content
├── system-design.md
├── tech-stack.md
├── patterns.md
├── contracts.md
├── conventions.md
└── common-pitfalls.md  (create empty if no content matches)
```

## Process

1. **Backup**: copy the monolith to `docs/kb.monolith.bak.<YYYYMMDD>.md` before doing anything.
2. **Read the monolith.** Identify section boundaries by H2 / H3 headings.
3. **Map sections to sub-docs** using these heuristics:
   - Headings containing "system", "architecture", "components", "boundaries", "data flow" → `system-design.md`
   - Headings containing "stack", "language", "framework", "library", "database", "DB", "infra", "hosting" → `tech-stack.md`
   - Headings containing "pattern", "convention" (architectural), "approach", "strategy", "auth flow", "error handling" → `patterns.md`
   - Headings containing "contract", "API", "endpoint", "schema", "data model", "type", "interface" → `contracts.md`
   - Headings containing "naming", "folder", "layout", "code style", "format", "lint", "test" (conventions only, not test docs) → `conventions.md`
   - Headings containing "pitfall", "gotcha", "common mistake", "anti-pattern" → `common-pitfalls.md`
   - Anything else → keep at the bottom of `system-design.md` under a `## Other` section; flag in the split report so kb-curator (or the user) can re-home it later.
4. **Write each sub-doc**. Each file starts with one H1 (the sub-doc's title), then the migrated content under their original H2/H3 headings. Preserve all text verbatim — no rewording, no truncation.
5. **Build `docs/kb/README.md`** as a pointer index. One line per sub-doc, format:
   ```
   - [System design](system-design.md) — components, responsibilities, data flow, boundaries
   - [Tech stack](tech-stack.md) — languages, frameworks, libs, DB, infra
   - [Patterns](patterns.md) — architectural and code patterns to follow
   - [Contracts](contracts.md) — API surface, data models, shared types
   - [Conventions](conventions.md) — naming, folder layout, testing approach
   - [Common pitfalls](common-pitfalls.md) — recurring reviewer findings to avoid
   ```
   Do not inline content into the index.
6. **Verify** no content was lost:
   - Sum the line counts of all new sub-docs (excluding their H1 titles).
   - Compare to the monolith line count (excluding backed-up frontmatter, if any).
   - Difference should be small and explainable (headings rewritten, blank-line normalization). If anything substantive is missing → restore the backup and ask the user.
7. **Delete the monolith** only after verification passes. The backup at `docs/kb.monolith.bak.<date>.md` stays for safety.
8. **Update references**: scan the repo for links to the old monolith path (`grep -rl <old-path> .`). Rewrite them to point to the matching sub-doc or to `docs/kb/README.md`.

## Output

Print a split report to the user:

```
KB split complete.
  Source:        docs/kb.md (700 lines)
  Backup:        docs/kb.monolith.bak.2026-05-24.md
  New structure: docs/kb/
    system-design.md   180 lines
    tech-stack.md       95 lines
    patterns.md        140 lines
    contracts.md       150 lines
    conventions.md      90 lines
    common-pitfalls.md  45 lines
    README.md            7 lines
  Sections needing review: <list any "Other" entries>
  References updated: <count>
```

Log the operation:

```
[<ts>] [kb-split] [KB split] from=<src> to=docs/kb/ sub-docs=6 lines=<total>
```

## Rules

- Backup before any destructive step. Never edit the monolith in place.
- Preserve content verbatim. This command does not rewrite or summarize.
- Do not invent sub-docs not in the target structure.
- If you cannot confidently classify a section, put it under `Other` in `system-design.md` and surface it — do not guess silently.
- Do not delete the backup file. The user removes it manually when comfortable.
