# Epic 6 — Toasts and Notifications

---

### T6.1 [frontend] toasts store + Toast component

- **Description**: Zustand store with a bounded toast queue (max 4 visible, FIFO). Toast component per UI §5.9 with progress bar for timed dismissal.
- **Acceptance**:
  - Success/Stopped toasts auto-dismiss in 8s; failed persist until dismissed.
  - Multiple toasts stack and animate per UI §5.9.
- **Dependencies**: T0.5.

---

### T6.2 [backend] toast:show on run terminal events

- **Description**: On `run:finished`, emit `toast:show` with kind=completed/failed/stopped, the sequence + project names, and the `run_id`.
- **Acceptance**:
  - Each terminal state emits exactly one toast.
- **Dependencies**: T4.3, T6.1.

---

### T6.3 [frontend] toast click navigation

- **Description**: Clicking a toast routes to the run view (S-05 if terminal, S-04 if still accessible).
- **Acceptance**:
  - Routing works from any screen.
- **Dependencies**: T6.1, T5.6.
