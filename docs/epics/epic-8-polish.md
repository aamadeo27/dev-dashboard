# Epic 8 — Polish and Cross-Cutting

---

### T8.1 [shared] Error translation utility

- **Description**: `utils/errors.ts` maps `AppError` `code`s to user-facing strings. Components display via `formatError(err)` only.
- **Acceptance**:
  - Every `AppError` variant has a translation.
  - Unknown codes fall back to a generic "Something went wrong (CODE: XYZ)".
- **Dependencies**: T0.2.

---

### T8.2 [frontend] Empty/loading/error states audit

- **Description**: Sweep every screen and component for the states enumerated in UI §7. Add missing skeletons, empty states, and error states.
- **Acceptance**:
  - Each row in the UI §7 table is reproducible by flipping a fixture flag.
- **Dependencies**: all major UI tasks.

---

### T8.3 [backend] Window focus/blur bridge

- **Description**: Wire Tauri window focus/blur/show/hide events to the `GitPoller`, `UsageProbe`, and `cli:lost` checker so they pause/resume.
- **Acceptance**:
  - Polling logs show no activity while window is blurred.
  - Resuming on focus triggers an immediate poll.
- **Dependencies**: T2.3, T7.1, T1.6.

---

### T8.4 [backend] Graceful shutdown

- **Description**: On window close, send cancel to all RunSession tokens. Wait up to 2s for cleanup, then force-exit. Finalize meta.json for any still-running runs as `failed` with note "App shutdown".
- **Acceptance**:
  - Closing during an active run leaves a coherent meta.json + transcript.jsonl on next launch.
- **Dependencies**: T4.3.

---

### T8.5 [frontend] Keyboard shortcuts and accessibility pass

- **Description**: Esc closes modals; Enter submits primary action; focus rings visible (UI tokens); reduced-motion respected. Tab order audited on each screen.
- **Acceptance**:
  - All modals dismissable via Esc.
  - No focus traps.
  - Reduced-motion disables all transitions.
- **Dependencies**: T5.8, T2.7.

---

### T8.6 [shared] README and developer onboarding

- **Description**: Top-level README: prerequisites, install, dev, build, test, troubleshooting. Brief architecture pointer to KB.
- **Acceptance**:
  - A fresh dev can clone and reach a running app in <10 minutes following the README.
- **Dependencies**: T0.1.

---

### T8.7 [shared] NFR verification smoke test

- **Description**: Manual verification of NFR-4 (visible feedback within 200ms of launch_run), NFR-5 (parsed events appear within 250ms of CLI emission), NFR-6 (idle RAM <200MB, CPU <1% with 20 projects). Document pass/fail results. Gate v1 release on PASS.
- **Acceptance**:
  - T4.3's acceptance criteria explicitly asserts run launch feedback <200ms.
  - T4.1's acceptance criteria asserts first parsed event emitted within 250ms of receiving first CLI output byte.
  - T8.7 manual checklist completed and checked in.
- **Dependencies**: T4.1, T4.3, T5.3 (all streaming tasks complete).
