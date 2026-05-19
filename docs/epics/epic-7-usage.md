# Epic 7 — Usage / Rate Limit

---

### T7.1 [backend] UsageProbe

- **Description**: Runs `<claude> /usage` as a subprocess (short timeout, 10s). Parses key-value lines from stdout into a `BTreeMap`. Schedules every 60s + on-demand. Pauses on window blur. Emits `usage:updated`.
- **Acceptance**:
  - Mock CLI returning known KV output -> snapshot reflects keys.
  - Subprocess failure -> `available=false`, error logged, no exception.
- **Dependencies**: T1.2.

---

### T7.2 [frontend] RateLimitPill + useUsage hook

- **Description**: Pill in Dashboard top bar (UI §5.2). Clicking opens a popover with full KV list and a Refresh button calling `refresh_usage`.
- **Acceptance**:
  - "--" state when `available=false`.
  - Spinner during refresh.
  - Updates within 500ms of `usage:updated` event.
- **Dependencies**: T7.1, T2.6.
