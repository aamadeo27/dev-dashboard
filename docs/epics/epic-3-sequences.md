# Epic 3 — Sequences

Unblocks Project Detail sequences panel and Launch Modal.

---

### T3.1 [backend] SequenceLoader

- **Description**: `list_sequences(project_id)`, `refresh_sequences(project_id)`. Reads `<project>/.claude/sequences/*.md`. Extracts description (first non-heading paragraph; `(No description)` fallback). Caches in-memory keyed by project_id; invalidates on directory mtime change.
- **Acceptance**:
  - Empty dir -> empty Vec, no error.
  - Description extraction handles: heading-only files, blank files, multi-paragraph files, Windows line endings.
  - Cache invalidates within one call after mtime change.
- **Dependencies**: T2.1.

---

### T3.2 [frontend] useSequences hook + SequenceRow component

- **Description**: Hook wraps `list_sequences`. Component renders name, description, [Run] button. Selected + hover states.
- **Acceptance**:
  - Empty state matches UI §5.3 / §5.6.
  - Long descriptions wrap cleanly within the card width.
- **Dependencies**: T3.1, T0.5.
