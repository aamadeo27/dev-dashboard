# Epic 1 — Settings and CLI Detection

Unblocks S-01 (Setup) and S-07 (Settings).

---

### T1.1 [backend] SettingsStore

- **Description**: Implement load/save for `settings.json` in OS config dir. Validate ranges (KB §3.5). Provide a `SettingsPatch` for partial updates. Atomic write (tmp file + rename).
- **Acceptance**:
  - First launch creates a default settings file.
  - `get_settings` and `update_settings` commands work.
  - Corrupted settings file -> falls back to defaults, logs a warning, archives the broken file with `.broken` suffix.
- **Dependencies**: T0.2, T0.6.

---

### T1.2 [backend] verify_claude_cli command

- **Description**: Resolve the CLI path (override > PATH lookup), spawn `<path> --version`, parse output, return `CliCheck`. Interactive-mode invocation (no `--print`, no `--output-format`) is the v1 contract — no additional flag probing required.
- **Acceptance**:
  - Returns `found=true` with version string when CLI is installed.
  - Returns `found=false` with a helpful error when missing.
  - Logs the resolved path at `info` level.
- **Dependencies**: T1.1.

---

### T1.3 [frontend] S-01 Setup screen

- **Description**: Implement UI spec §5.1. On app load, call `verify_claude_cli(undefined)`; if not found, route to `/setup`. The setup screen shows OS-detected install instructions, a path input with Browse, and a Verify button calling `verify_claude_cli(path)`.
- **Acceptance**:
  - All states from UI §5.1 (Initial, Verifying, Success, Failure) render correctly.
  - Successful verification persists the path via `update_settings({ claude_cli_path })` and routes to `/`.
  - Browse button opens an OS file picker filtered to executables.
- **Dependencies**: T1.2, T0.5.

---

### T1.4 [frontend] S-07 Settings screen

- **Description**: Implement UI spec §5.7. Form-bound to `Settings`. Save calls `update_settings`. Includes "Open logs folder" button (calls a new `open_logs_folder` command). Unsaved-changes prompt on back.
- **Acceptance**:
  - All fields from UI §5.7 are present with described validation.
  - Saved confirmation appears for 2s after successful save.
  - View toggle here stays in sync with Dashboard toolbar (shared via Zustand `ui` store).
- **Dependencies**: T1.1, T0.5.

---

### T1.5 [backend] open_logs_folder command

- **Description**: Use `tauri-plugin-opener` to reveal the log folder in OS file manager.
- **Acceptance**: Works on Win/macOS/Linux. Errors surface as `AppError::Io`.
- **Dependencies**: T0.6.

---

### T1.6 [backend] cli:lost detection

- **Description**: Background task that re-runs `verify_claude_cli` on a 60s interval (paused when window unfocused). On transition from found -> not-found, emit `cli:lost` event.
- **Acceptance**:
  - Renaming the CLI binary mid-session emits the event within ~60s.
  - Restoring it stops further `cli:lost` emissions on next check.
- **Dependencies**: T1.2.
