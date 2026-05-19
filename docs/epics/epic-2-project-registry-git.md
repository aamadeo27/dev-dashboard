# Epic 2 — Project Registry and Git Status

Unblocks S-02 (Dashboard).

---

### T2.1 [backend] ProjectRegistry CRUD

- **Description**: Implement `list_projects`, `add_project`, `remove_project`, `relocate_project`, `set_project_tags`, `rename_project`. Persist to `projects.json` atomically. Canonicalize paths. Reject duplicate paths.
- **Acceptance**:
  - Adding the same path twice returns `AppError::AlreadyExists`.
  - Tags are lowercased, trimmed, deduped before save.
  - `list_projects` populates `is_missing` by `path.exists()` check.
- **Dependencies**: T0.2.

---

### T2.2 [backend] ProjectScanner (language/PM detection)

- **Description**: On `add_project` and on demand, detect language and package manager via marker files: `Cargo.toml` -> rust/cargo; `package.json` + `pnpm-lock.yaml` -> ts/pnpm; etc. Update fields on the `Project`.
- **Acceptance**:
  - At least 6 stacks detected: rust/cargo, ts/pnpm, ts/npm, python/uv, python/poetry, go/gomod.
  - Unknown projects: `language=None, package_manager=None` — no error.
- **Dependencies**: T2.1.

---

### T2.3 [backend] GitPoller

- **Description**: Per-project git status via `git2`. Returns `GitStatus`. A central poller task tracks "visible project ids" set; polls each visible project at `git_poll_interval_secs`. Pauses on window blur. Emits `git:updated` events.
- **Acceptance**:
  - `get_git_status(id)` returns clean/dirty/ahead/behind correctly on a test repo.
  - Visible-set updates via a `set_visible_projects(ids: Vec<String>)` command from the frontend.
  - Polling pauses within 1s of window blur and resumes on focus.
- **Dependencies**: T2.1, T1.1.

---

### T2.4 [frontend] useProjects + useGitStatus hooks

- **Description**: TanStack Query hook backed by `list_projects`. `useGitStatus(id)` reads from a Zustand store kept in sync by `git:updated` events. `useVisibleProjects` reports visible card ids to backend via `set_visible_projects` (debounced).
- **Acceptance**:
  - Mounting the Dashboard registers visible project ids within 500ms.
  - Hiding the window (window:blur) reports an empty visible set.
- **Dependencies**: T2.3, T0.4.

---

### T2.5 [frontend] ProjectCard component

- **Description**: Implement UI §5.2 grid card. Includes status edge color, name, tag chips, git badge, last-run badge, quick-run button. Missing-state variant. Loading skeleton variant.
- **Acceptance**:
  - All five card states from UI §5.2 render against mock data.
  - Quick-run button shows correct tooltip per prior-run state.
- **Dependencies**: T0.5.

---

### T2.6 [frontend] S-02 Dashboard layout, toolbar, empty state

- **Description**: Top bar (logo, rate-limit pill placeholder, gear), toolbar (Add Project, search, tag filter chips, view toggle), grid/list switcher, empty state. Wire to `useProjects`.
- **Acceptance**:
  - Add Project opens a directory picker via `tauri-plugin-dialog` and calls `add_project`.
  - Search filters cards by name or path on each keystroke.
  - Tag filter chips reflect the union of all project tags.
  - View toggle persists via Settings.
- **Dependencies**: T2.5, T2.4, T1.4.

---

### T2.7 [frontend] Project card context menu + tag editor popover

- **Description**: Right-click context menu (Open in Editor, Open in Terminal, Edit Tags, Remove, Relocate-if-missing). Tag editor popover (S-08) anchored to card.
- **Acceptance**:
  - All five context items work.
  - Tag changes update the card in real time.
  - Remove shows inline confirm; cancel restores card.
- **Dependencies**: T2.6, T2.8.

---

### T2.8 [backend] open_in_editor / open_in_terminal commands

- **Description**: Implement per KB §6.7 / UI §5.2 / GAP-08: `$EDITOR` env var first; OS default file association fallback. Default terminal launch per OS.
- **Acceptance**:
  - Works on all three OSes (manual smoke).
  - Failure emits an error toast via `toast:show` and returns `AppError::Io`.
- **Dependencies**: T0.2.
