---
description: Break heavy KB sub-docs (patterns, conventions, contracts, tech-stack) into one-file-per-item folders with index README.
argument-hint: [sub-doc-name | all]
---

You are itemizing a Knowledge Base sub-doc into a folder of single-item files plus a tight index.

## Target sub-docs

Operates on these four (and only these):
- `docs/kb/patterns.md`
- `docs/kb/conventions.md`
- `docs/kb/contracts.md`
- `docs/kb/tech-stack.md`

These are the four that grow item-by-item and benefit from per-item reads. `system-design.md`, `common-pitfalls.md`, and `README.md` stay as single files.

## Input

`$ARGUMENTS` is the sub-doc to itemize:
- `patterns`, `conventions`, `contracts`, `tech-stack` — itemize one.
- `all` — itemize all four sub-docs in sequence.
- empty — refuse; ask the user which.

If the target folder (e.g., `docs/kb/patterns/`) already exists with a `README.md` → **stop**; that sub-doc is already itemized.

## Target structure (per sub-doc)

For `patterns.md` → `docs/kb/patterns/`:

```
docs/kb/patterns/
├── README.md                # item index — one line per item
├── <slug-1>.md              # one item per file
├── <slug-2>.md
└── ...
```

Same shape for `conventions/`, `contracts/`, `tech-stack/`.

## Process (per sub-doc)

1. **Backup**: copy the sub-doc to `docs/kb/<name>.bak.<YYYYMMDD>.md`.
2. **Identify items** by H2 / H3 headings. Each heading + its following content (up to the next heading of equal or higher level) becomes one item.
   - If the sub-doc has only H1 + a flat list, treat each top-level bullet or section as one item — but ask the user first if the structure is ambiguous.
3. **Slug each item**: lowercase-kebab from the heading. Collision → suffix with `-2`, `-3`.
4. **Write `docs/kb/<name>/<slug>.md`** for each item. Each file:
   - H1 = item title (the original heading)
   - Body = original content verbatim. No rewording, no truncation.
5. **Build `docs/kb/<name>/README.md`** as the item index. Format:
   ```
   # <Sub-doc Title>

   - [<Item title>](<slug>.md) — one-line description
   - [<Item title>](<slug>.md) — one-line description
   ```
   The one-line description should be ≤120 chars, derived from the item's first sentence or summary line. **Index has no content beyond this list.** Do not inline item bodies.
6. **Verify**: sum the line counts of all item files (excluding their H1 lines). Compare to the original sub-doc (excluding its H1 and any meta). Difference must be small and explainable. If anything substantive missing → restore backup, ask the user.
7. **Update the top-level `docs/kb/README.md`**: change the entry for this sub-doc from
   ```
   - [Patterns](patterns.md) — ...
   ```
   to
   ```
   - [Patterns](patterns/README.md) — ...
   ```
8. **Delete the monolithic sub-doc** only after verification passes. Backup stays.
9. **Update references**: scan the repo for inbound links to the old file (`grep -rl docs/kb/<name>.md .`). Rewrite to `docs/kb/<name>/README.md` (or to the specific item if obvious).

## Output

Print a per-sub-doc report:

```
Itemized docs/kb/patterns.md
  Backup:     docs/kb/patterns.bak.2026-05-24.md
  Folder:     docs/kb/patterns/
  Items:      12   (avg 38 lines each, max 90, min 11)
  Index:      14 lines
  References updated: 4
```

Log:

```
[<ts>] [kb-itemize] [Sub-doc itemized] name=<patterns> items=<n> lines=<total>
```

## Rules

- One item per file. No bundling, no double items.
- Preserve content verbatim. This is not a rewrite pass.
- Index is pointers only. Never inline item bodies into the index.
- Never delete the backup; user removes it manually when comfortable.
- Refuse if the folder already exists structured.
- If an item is > 300 lines after the split → flag it; that item is itself a candidate for a further sub-split (e.g., `patterns/<area>/<slug>.md`). Do not auto-split deeper; surface to the user.
