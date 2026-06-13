# Epic 5 — Run UI (Live and Historical)

---

### T5.1 [frontend] EventBlock components

- **Description**: One component per `RunEvent` variant (KB §4). Each implements the visual treatment from UI §5.4 (assistant markdown, thinking collapsible, tool call expandable, tool result, file edit diff, user input bubble, system, step failed).
- **Acceptance**:
  - Storybook-style harness page renders one of each, against fixture data committed to the repo.
  - Markdown sanitization in AssistantBlock (no script injection from CLI output).
  - DiffBlock highlights additions green, deletions red, context neutral.
- **Dependencies**: T0.5.

---

### T5.2 [frontend] liveRuns Zustand store + useLiveRun hook

- **Description**: Store keyed by `run_id` holds: events array, status, started_at, exit_code. `useLiveRun(run_id)` subscribes to `run:event`, `run:started`, `run:finished` filtered by id. Bounded buffer (10k events) with overflow log.
- **Acceptance**:
  - Mounting/unmounting a live run view does not leak event listeners.
  - Two live run views open simultaneously remain isolated.
- **Dependencies**: T0.4, T4.3.

---

### T5.3 [frontend] S-04 Run View Live

- **Description**: Implement UI §5.4. Header bar with Stop (inline confirm), event stream with auto-scroll + jump-to-bottom, persistent UserInputBox (Enter sends, Shift+Enter newline).
- **Acceptance**:
  - All states from UI §5.4 render correctly (pending, running, stopping, completed, failed, stopped).
  - UserInputBox sends via `send_input`; on terminal state, input box and Send disable.
  - Auto-scroll pauses on user scroll-up; Jump-to-bottom appears and works.
- **Dependencies**: T5.1, T5.2, T4.3.

---

### T5.4 [frontend] Step failure prompt UI

- **Description**: Inline within the event stream, on `run:step_failure` event, render a card with Retry / Skip / Abort / Continue (default highlighted). Clicking sends `respond_to_step_failure`. Auto-dismisses after 60s as Continue (with countdown indicator).
- **Acceptance**:
  - Buttons send the correct choice.
  - 60s countdown is visible.
  - Auto-Continue fires if untouched.
- **Dependencies**: T5.3, T4.7.

---

### T5.5 [backend] load_transcript + list_runs commands

- **Description**: `list_runs(project_id)` returns `Vec<Run>` from `meta.json` files, sorted newest-first. `load_transcript(run_id, project_id)` streams `transcript.jsonl` and returns the full event list (or an error if missing/corrupt).
- **Acceptance**:
  - 1000-event transcript loads in <500ms on a mid-range dev machine.
  - Corrupt JSONL line -> error variant `ParseError` with line number.
- **Dependencies**: T4.2.

---

### T5.6 [frontend] S-05 Run View Historical

- **Description**: Reuses `EventBlock` components from T5.1. Loads transcript via `load_transcript`. No Stop button, no UserInputBox, shows duration. Error state with [Open folder] when transcript unavailable.
- **Acceptance**:
  - Renders a completed run identically to its live view.
  - Open folder reveals the run dir in OS file manager.
- **Dependencies**: T5.1, T5.5.

---

### T5.7 [frontend] S-03 Project Detail

- **Description**: UI §5.3. Header with Launch Sequence button, git status bar, two-panel layout: Run History (uses `list_runs`) and Sequences (uses `list_sequences`). Sequences panel focus-and-pulse mode when entered from quick-run with no prior run.
- **Acceptance**:
  - Clicking a run row routes to S-04 (if still running) or S-05.
  - Sequences panel highlight pulse runs for 2s and fades.
  - Refresh icon in git status bar calls `refresh_git_status`.
- **Dependencies**: T3.2, T5.5, T2.6.

---

### T5.8 [frontend] S-06 Launch Modal

- **Description**: UI §5.6. Sequence selector + optional .md attach. Pre-fill when entered from quick-run with prior run. Launch button calls `launch_run` and navigates to the live run view.
- **Acceptance**:
  - Backdrop click and Esc close the modal.
  - Attached file chip shows filename; × removes attachment.
  - Launch failure shows the error banner without closing the modal.
- **Dependencies**: T3.2, T4.3.

---

### T5.9 [frontend] Background run badge on project card

- **Description**: When `run:event`/`run:started`/`run:finished` events arrive for runs whose project is visible, the card shows a "Running" badge (or "N running" if >1). Clicking the badge navigates to the live run view (or shows a small picker if multiple).
- **Acceptance**:
  - Starting a run while on the Dashboard updates the card's badge within 500ms.
  - Multiple runs on the same project show the count and the picker.
- **Dependencies**: T5.2, T2.5.

---

### T5.10 [frontend] Wire Dashboard quick-run dispatch

> [adoption-assumption] Added during adoption (2026-06-13). `Dashboard.handleQuickRun` is currently a no-op (`TODO` comment) — AS-BUILT divergence #14 in `docs/ui-ux.md` Appendix B. The destination screens (S-03 panel-focus in T5.7, S-06 pre-fill in T5.8) and the badge click (T5.9) exist as separate tasks, but the Dashboard-side dispatch glue that routes the ⚡ button to the correct destination based on prior-run state has no owner. This task closes that seam.

- **Description**: Implement the `handleQuickRun(projectId)` dispatch in `src/routes/Dashboard.tsx` per UI §3 (Navigation Map) and §4 (Flow Map "Quick-run" rows). Resolve the project's last-run state via the run/history source, then: (a) no prior run → navigate to S-03 Project Detail with the Sequences panel focus-and-pulse flag set (consumed by T5.7); (b) prior run exists → open S-06 Launch Modal pre-filled with the last sequence (consumed by T5.8). Respect disabled/loading states already present on the ⚡ button (missing project → disabled; loading → no-op). Remove the `TODO` no-op.
- **Acceptance**:
  - ⚡ on a card with no prior run navigates to S-03 and the Sequences panel receives the focus-and-pulse signal.
  - ⚡ on a card with a prior run opens S-06 pre-filled with the last sequence.
  - ⚡ on a missing project remains disabled; ⚡ while run data is loading is a no-op (no navigation).
  - No remaining `TODO`/no-op in `handleQuickRun`.
- **Dependencies**: T5.7, T5.8.
- **kb-refs**:
  ```
  patterns:    [layering]
  contracts:   [ipc-runs, ipc-sequences]
  conventions: [naming, file-layout, testing-approach]
  tech-stack:  [frontend-react-ts-vite, state-management]
  ```
