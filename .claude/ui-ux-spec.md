# UI/UX Specification: Dev Dashboard

**Project**: Dev Dashboard
**Author**: aamadeo@gmail.com
**Date**: 2026-05-18
**Status**: Final — no open gaps.

---

## Table of Contents

1. [Design System](#1-design-system)
2. [Screen Inventory](#2-screen-inventory)
3. [Navigation Map](#3-navigation-map)
4. [Flow Map](#4-flow-map)
5. [Screen Specifications](#5-screen-specifications)
   - 5.1 Setup Screen (CLI Missing)
   - 5.2 Dashboard (Project List)
   - 5.3 Project Detail View
   - 5.4 Run View (Live)
   - 5.5 Run View (Historical)
   - 5.6 Launch Sequence Modal
   - 5.7 Settings Screen
   - 5.8 Tag Editor Popover
   - 5.9 Toast Notification System
6. [Component Library](#6-component-library)
7. [Edge Cases and States](#7-edge-cases-and-states)
8. [Gaps](#8-gaps)

---

## 1. Design System

### 1.1 Color Palette

The application runs locally and is developer-facing. The palette is dark-first (developers prefer dark UIs for long sessions), with a cool-neutral base, a vivid violet primary (distinctive, not a commodity blue), and a warm amber accent.

| Token | Hex | Usage |
|---|---|---|
| `--bg-base` | `#0F1117` | Main window background |
| `--bg-surface` | `#1A1D27` | Cards, panels, sidebars |
| `--bg-elevated` | `#22263A` | Modals, popovers, dropdowns |
| `--bg-hover` | `#2A2F47` | Hover state on interactive surfaces |
| `--border-subtle` | `#2E334D` | Dividers, card borders |
| `--border-strong` | `#4A5080` | Focused inputs, active states |
| `--primary` | `#7C6AF7` | Primary actions, active nav |
| `--primary-hover` | `#9182F9` | Primary button hover |
| `--primary-dim` | `#3D3578` | Primary tint backgrounds |
| `--accent` | `#F5A623` | Quick-run button, highlights |
| `--accent-hover` | `#F7B84B` | Accent hover |
| `--text-primary` | `#E8EAF2` | Body text, labels |
| `--text-secondary` | `#8A90B0` | Timestamps, metadata |
| `--text-disabled` | `#4A5070` | Disabled controls |
| `--text-on-primary` | `#FFFFFF` | Text on primary-colored surfaces |
| `--success` | `#3DD68C` | Completed status, clean git |
| `--success-dim` | `#1A4A35` | Success tint backgrounds |
| `--warning` | `#F5A623` | Ahead/behind git, warnings |
| `--warning-dim` | `#4A3A10` | Warning tint backgrounds |
| `--error` | `#F2535A` | Failed status, errors, dirty git |
| `--error-dim` | `#4A1A1D` | Error tint backgrounds |
| `--stopped` | `#8A90B0` | Stopped run status |
| `--running` | `#7C6AF7` | Running status indicator |
| `--thinking` | `#9182F9` | Thinking block highlight |
| `--tool-call` | `#3ABFCF` | Tool call event highlight |
| `--file-edit` | `#F5A623` | File edit event highlight |

**Rationale**: Violet primary avoids the over-used blue and reads as "intelligent/AI" without being trendy purple. Amber accent contrasts without clashing on the dark base. Semantic colors follow conventional green/amber/red conventions for immediate recognition.

### 1.2 Typography

| Role | Font | Size | Weight |
|---|---|---|---|
| App name / screen title | System UI sans-serif | 18px | 700 |
| Section heading | System UI sans-serif | 14px | 600 |
| Body / labels | System UI sans-serif | 13px | 400 |
| Metadata / timestamps | System UI sans-serif | 11px | 400 |
| Code / paths / diffs | System monospace | 12px | 400 |
| Run event text | System monospace | 12px | 400 |

Line heights: body 1.5, code 1.6. No web fonts — system fonts ensure instant render and platform feel.

### 1.3 Spacing Scale

`4 / 8 / 12 / 16 / 24 / 32 / 48px` — base 4px grid throughout.

### 1.4 Border Radius

Cards: `8px`. Buttons: `6px`. Inputs: `6px`. Tags/chips: `4px`. Popovers: `8px`. Toasts: `8px`.

### 1.5 Iconography

Single icon set (e.g., Phosphor Icons or Lucide), 16px default, 20px for toolbar icons. No custom SVG unless unavoidable. Icons always paired with a tooltip or visible label for discoverability.

### 1.6 Motion

Transitions: `150ms ease-out` for hover/focus. Modal open: `200ms ease-out` scale + fade. Toast slide-in: `200ms ease-out` from bottom-right. No animation exceeds `300ms`. Reduced motion media query honored — all animation disabled when OS preference is set.

---

## 2. Screen Inventory

| ID | Screen Name | Purpose |
|---|---|---|
| S-01 | Setup Screen | Shown when Claude CLI is not found on PATH; blocks launch |
| S-02 | Dashboard | Primary screen; grid of project cards; entry to all project actions |
| S-03 | Project Detail | Per-project view: runs history + sequences panel |
| S-04 | Run View (Live) | Streams a running sequence in real time |
| S-05 | Run View (Historical) | Re-renders a completed run from stored transcript |
| S-06 | Launch Sequence Modal | Two-step picker: choose sequence + optional file attach |
| S-07 | Settings | App-wide configuration |
| S-08 | Tag Editor Popover | Inline tag management on project card (right-click context) |
| S-09 | Toast System | Overlay notification layer (not a screen; always present) |

---

## 3. Navigation Map

```
App Launch
    |
    +-- Claude CLI missing? --> [S-01 Setup Screen]
    |       |
    |       +-- CLI found / path set --> [S-02 Dashboard]
    |
    +-- CLI present --> [S-02 Dashboard]
            |
            +-- Click project card --> [S-03 Project Detail]
            |       |
            |       +-- Click run row --> [S-05 Run View Historical]
            |       +-- Click "Launch" button --> [S-06 Launch Modal]
            |       |       |
            |       |       +-- Confirm --> [S-04 Run View Live]
            |       |
            |       +-- Active run badge --> [S-04 Run View Live]
            |
            +-- Click ⚡ on card (no prior run) --> [S-03 Project Detail, Sequences panel focused]
            +-- Click ⚡ on card (has prior run) --> [S-06 Launch Modal] (re-run last sequence)
            +-- Click [+ Add Project] --> directory picker dialog --> [S-02 Dashboard]
            +-- Right-click project card --> context menu
            |       |
            |       +-- "Edit Tags..." --> [S-08 Tag Editor Popover]
            |       +-- "Remove Project" --> confirmation inline
            |       +-- "Open in Editor" --> launches $EDITOR / OS association (no screen)
            |       +-- "Open in Terminal" --> launches OS default terminal (no screen)
            |
            +-- Click gear icon --> [S-07 Settings]
            +-- Click toast --> [S-04 or S-05 Run View]
```

Every screen is reachable from the map above. No orphan screens.

---

## 4. Flow Map

| User Action | Entry Point | Screens Traversed | Result |
|---|---|---|---|
| Register project | Dashboard → "+ Add Project" | S-02 → OS directory picker → S-02 | New card appears in grid |
| Remove project | Dashboard → right-click card → "Remove Project" | S-02 → inline confirm | Card removed; files untouched |
| View project detail | Dashboard → click card | S-02 → S-03 | Project detail opens |
| Launch sequence | Project Detail → "Launch" OR Dashboard → Launch modal | S-03 / S-02 → S-06 → S-04 | Live run view opens |
| Quick-run (no prior run) | Dashboard → ⚡ button on card | S-02 → S-03 (Sequences panel focused) | User selects sequence, then → S-06 → S-04 |
| Quick-run (prior run exists) | Dashboard → ⚡ button on card | S-02 → S-06 (pre-filled last sequence) → S-04 | Live run view opens |
| Attach .md file | Launch Modal → "Attach file" | S-06 → OS file picker → S-06 | File name shown in modal |
| Watch live run | Any path to S-04 | S-04 | Streaming events, stop button |
| Stop run | Live Run View → Stop | S-04 | Run marked stopped; stop button disabled |
| View past run | Project Detail → run row | S-03 → S-05 | Historical run rendered |
| Edit project tags | Dashboard → right-click card → "Edit Tags..." | S-02 → S-08 popover | Tags updated on card |
| Open in editor | Dashboard → right-click card → "Open in Editor" | S-02 → OS ($EDITOR / file assoc.) | Editor opens; app stays open |
| Open in terminal | Dashboard → right-click card → "Open in Terminal" | S-02 → OS default terminal | Terminal opens at project dir |
| Configure settings | Dashboard → gear icon | S-02 → S-07 | Settings saved |
| CLI missing | App launch | S-01 | User sets CLI path or installs |
| Toast click | Any screen | Any → S-04 or S-05 | Relevant run view opens |

---

## 5. Screen Specifications

---

### 5.1 Setup Screen (S-01)

**Purpose**: Block app use when Claude CLI is not found, provide actionable instructions.

**Entry**: App launch when `claude` is not found on PATH.

**Layout**: Full-window centered card, max-width 520px, vertically centered.

**Components**:

- App logo / name at top of card
- Heading: "Claude CLI not found"
- Body text: "The Dev Dashboard requires the Claude CLI (`claude`) to be available on your PATH. Install it or set a custom path below."
- Installation instructions block (monospace code block):
  - Shows the install command for the detected OS
  - Windows: `winget install Anthropic.Claude` (or npm equivalent per actual CLI docs)
  - macOS/Linux: `npm install -g @anthropic-ai/claude-code` (or brew)
- Divider labeled "Or set a custom path"
- Text input: label "Claude CLI path", placeholder `/usr/local/bin/claude`, full width
- Button row: [Browse...] [Verify & Continue] (primary, full-width on small)
- Status area below buttons: shows "Verifying..." spinner when checking, green check + "Found: version X.Y.Z" on success, red error text on failure

**States**:

| State | UI |
|---|---|
| Initial | Instructions shown, input empty |
| Path entered, not verified | "Verify & Continue" enabled |
| Verifying | Spinner in button; input disabled |
| Verification success | Green check, version shown; button label becomes "Continue" |
| Verification failure | Red error: "Could not run `<path> --version`. Check the path." |

**Interactions**:
- "Browse..." opens OS file picker filtered to executables.
- "Verify & Continue" (when success): transitions to S-02 Dashboard with a slide animation.
- Once on S-02, if CLI disappears (e.g., uninstalled mid-session), a non-dismissable banner replaces the top bar with the same error and a "Fix" link returning to S-01.

---

### 5.2 Dashboard (S-02)

**Purpose**: Main hub. Shows all registered projects as cards. Entry point for all primary actions.

**Layout**:
```
[Top Bar: App name | Rate limit indicator | Gear icon]
[Toolbar: "+ Add Project" | Search input | Tag filter chips | View toggle (grid/list)]
[Project Grid / List]
[Toast Layer (overlay, bottom-right)]
```

**Top Bar**:
- Left: App name "Dev Dashboard" (non-interactive logo/text)
- Center-right: Rate limit indicator (see below)
- Right: Gear icon button — opens S-07 Settings

**Rate Limit Indicator** (resolves GAP-01):

The app runs `claude /usage` as a subprocess and parses stdout on a schedule (every 60 seconds and on window focus). The indicator is a compact pill in the top bar:

- Icon (gauge/meter icon) + text: "Tokens: 45.2k / 100k" (or whatever parsed fields are available)
- If usage data is unavailable or subprocess fails: pill shows "--" with a tooltip "Usage data unavailable"
- Clicking the pill opens a small popover with the full parsed output of `claude /usage` formatted as a key-value list, and a "Refresh" button that re-runs the subprocess immediately
- While refreshing: spinner replaces icon in pill

**Toolbar**:
- "+ Add Project" button (secondary style): triggers OS directory picker. On selection, project is registered immediately and card appears. If directory already registered, shows inline tooltip error "Already added".
- Search input (text): filters project cards by name or path as you type. Placeholder "Search projects...". Clear (×) button appears when non-empty.
- Tag filter chips: horizontal scrollable strip of all unique tags in use across projects. Each chip is a toggle; clicking activates filter to show only projects with that tag. Multiple chips = AND filter. "Clear filters" link appears when any chip is active.
- View toggle: two icon buttons — grid view (default) and list view. State persisted in settings.

**Project Card (Grid View)**:

Each card is a surface with the following zones:

```
[Card border: colored left-edge by git status]
[Header row: Project name (bold) | ⚡ quick-run button]
[Tag chips row (if any tags assigned)]
[Git row: branch icon + branch name | status badge]
[Meta row: last run timestamp + outcome badge | path (truncated, tooltip)]
[Footer: "Missing" banner if path gone (replaces above)]
```

- Card click (anywhere except interactive controls): navigates to S-03 Project Detail.
- ⚡ quick-run button (amber accent, top-right of card):
  - If no prior run exists for this project: navigates to S-03 Project Detail with the Sequences panel scrolled into view and visually highlighted (pulsing border for 2 seconds) to guide the user. (Resolves GAP-03.)
  - If at least one prior run exists: opens S-06 Launch Modal pre-populated with the last sequence used.
  - Tooltip: "Quick-run last sequence" (or "Pick a sequence" if no prior run).
- Git status left-edge colors: `--success` for clean, `--error` for dirty, `--warning` for ahead/behind, `--text-disabled` for unknown/loading.
- Git status badge text: "Clean" / "Dirty" / "Ahead 2" / "Behind 3" / "Ahead 1 · Behind 2" / "Loading..." / "Error".
- Last run outcome badge: colored pill — "Completed" (success), "Failed" (error), "Stopped" (stopped), "Running" (primary, animated pulse), "Never" (text-secondary, no pill).
- If a run is currently active for this project, the "Running" badge in the card is clickable and leads directly to S-04.
- Right-click on card → context menu:
  - "Open in Editor" — launches $EDITOR env var with project path; if $EDITOR unset, uses OS default file association for directory (resolves GAP-08)
  - "Open in Terminal" — launches OS default terminal at project path (resolves GAP-08)
  - "Edit Tags..." — opens S-08 Tag Editor Popover anchored to the card
  - "Remove Project" — shows inline confirmation replacing card content: "Remove [name] from dashboard?" with [Cancel] [Remove] buttons. Destructive confirm is red. Does not delete files.
  - Separator
  - "Relocate..." — only shown if project is in "missing" state; opens OS directory picker to re-point the project path.

**Project Card (List View)**:

Single row per project:
```
[Status edge | Name | Path | Branch | Git status | Last run | ⚡]
```
Same interactions as grid; right-click same context menu.

**Empty State** (no projects registered):

Centered in grid area:
- Large icon (folder with plus)
- Heading: "No projects yet"
- Body: "Add a local project directory to get started."
- Button: "+ Add Project" (primary)

**Loading State** (initial app load, projects loading from registry):

Skeleton cards (gray shimmer placeholders) matching the card layout.

**Missing Project State**:

Card renders normally but:
- Left edge is gray (`--text-disabled`)
- Name struck-through or grayed
- Git row replaced with: warning icon + "Directory not found"
- Footer shows: [Relocate...] [Remove] buttons instead of normal footer
- ⚡ button is disabled (tooltip: "Project directory missing")

---

### 5.3 Project Detail (S-03)

**Purpose**: Per-project hub showing run history and available sequences. Central entry point for launching and reviewing.

**Entry**: Click project card on Dashboard.

**Layout**:
```
[Back button "<- Dashboard" | Project name (heading) | [Launch Sequence] button]
[Git status bar: branch | status | last polled timestamp | [Refresh] icon]
[Two-column layout or stacked on narrow:]
  [Left/Top: Run History panel]
  [Right/Bottom: Sequences panel]
```

**Back button**: Returns to S-02 Dashboard (preserves scroll position).

**Git status bar**: Same data as card but more verbose. Example: "main · Dirty (3 files changed) · Ahead 2 · Last updated 12s ago". [Refresh] icon triggers immediate poll.

**[Launch Sequence] button** (primary): Opens S-06 Launch Modal.

**Run History Panel**:

- Section heading: "Run History"
- Sorted newest-first list. Each row:
  - Outcome icon (colored) + Sequence name + "against [project]" (omit project if obvious)
  - Start timestamp (relative: "2 hours ago") + duration ("3m 42s")
  - Status badge
  - Clicking a row: navigates to S-05 (historical) or S-04 (if still running)
- If no runs: empty state — "No runs yet. Launch a sequence to get started."
- Loading state: 3 skeleton rows

**Sequences Panel** (resolves GAP-03 for the focused/highlighted state):

- Section heading: "Available Sequences"
- Lists all sequences from the app data directory.
- Each sequence row:
  - Name (bold)
  - Description: first non-heading paragraph from the sequence's `.md` file (resolves GAP-06). If no paragraph found, show "(No description)".
  - [Run] button (secondary): opens S-06 Launch Modal pre-populated with this sequence.
- If sequences panel is focused/highlighted (entered via ⚡ with no prior run): the panel has a pulsing `--primary` border for 2 seconds, and the panel scrolls into view automatically. After 2 seconds the highlight fades.
- If no sequences exist: empty state — "No sequences found. Add sequence files to [config dir path]."

---

### 5.4 Run View — Live (S-04)

**Purpose**: Real-time streaming view of an active run. Shows parsed events, exposes Stop control, accepts user input when prompted.

**Entry**: Confirm in S-06 Launch Modal, or click active run badge on project card, or click active run row in S-03.

**Layout**:
```
[Header bar: <- back | Sequence name @ Project name | Status badge (Running, pulse) | [Stop] button]
[User Input Box (always visible during run — see below)]
[Event Stream (scrollable, grows downward)]
[Auto-scroll toggle (bottom-right corner of stream area)]
```

**Header bar**:
- Back button: returns to S-03 Project Detail. Run continues in background. Badge appears on project card.
- Sequence name @ project name: non-interactive label.
- Status badge: animated "Running" pill.
- [Stop] button: destructive (red outline). Single click: confirms inline — button text changes to "Confirm Stop?" with [Yes, Stop] / [Cancel] for 3 seconds, then reverts. On confirm: sends kill signal to child process, status transitions to "stopped", stop button disabled.

**User Input Box** (resolves GAP-02):

Always rendered and active during a run (not conditional on CLI prompt detection — protocol deferred to architect). Positioned above the event stream (pinned).

- Multi-line text area, min-height 48px, max-height 120px (auto-grows within range)
- Placeholder: "Send input to the running process..."
- [Send] button (primary, right of textarea): sends content to child process stdin + appends as a "user" event in the stream. Disabled when textarea is empty.
- Keyboard: Enter submits (sends). Shift+Enter inserts newline.
- After sending: textarea clears, focus returns to textarea.
- When run ends (any terminal state): textarea and Send button are both disabled; placeholder changes to "Run has ended."

**Event Stream**:

Events rendered in order of emission. Each event is a distinct visual block separated by a thin divider:

| Event Type | Visual Treatment |
|---|---|
| Assistant text | White/`--text-primary`, plain prose, markdown rendered (bold, code spans, lists) |
| Thinking block | Collapsible accordion. Header: purple/`--thinking` left-border + "Thinking..." label + chevron. Content: italic monospace text. Collapsed by default. |
| Tool call | Teal/`--tool-call` left-border block. Header: tool icon + tool name + "(running...)" spinner while pending, checkmark when done. Body (expandable): input params as formatted JSON. |
| Tool result | Indented under tool call, lighter border. Shows result summary; full result in expandable detail. |
| File edit | Amber/`--file-edit` left-border block. Header: file icon + file path + lines changed. Body: unified diff with green/red line coloring (view-only). |
| User input sent | Right-aligned bubble, `--primary-dim` background. Timestamp. |
| System/status | Centered gray text (e.g., "Run started", "Run stopped by user"). |

Auto-scroll: stream auto-scrolls to bottom as events arrive. If user manually scrolls up, auto-scroll pauses and a "Jump to bottom" floating button appears (bottom-right). Clicking it re-enables auto-scroll.

**States**:

| State | UI change |
|---|---|
| Run starting (pending) | "Starting..." spinner in header, no events yet |
| Running | Full UI as described |
| Stopping | [Stop] button disabled, "Stopping..." in header |
| Completed | Status badge changes to "Completed" (green). Input box disabled. |
| Failed | Status badge changes to "Failed" (red). Error event block shown last with stderr output. Input box disabled. |
| Stopped | Status badge "Stopped" (gray). Input box disabled. |

---

### 5.5 Run View — Historical (S-05)

**Purpose**: Re-renders a past run from stored JSONL transcript (resolves GAP-10: transcript at `<project>/.claude/runs/<run-id>/transcript.jsonl`).

**Entry**: Click a past run row in S-03 Project Detail.

**Layout**: Identical to S-04 except:
- Header status badge shows final outcome (Completed / Failed / Stopped), no animation.
- No [Stop] button; instead shows run duration: "Duration: 3m 42s".
- No User Input Box (run is over).
- Full event stream shown immediately (loaded from JSONL), no streaming delay.
- Loading state while parsing JSONL: spinner centered in stream area.
- If JSONL file is missing or corrupt: error state — "Transcript unavailable. The file at `<path>` could not be read." with [Open folder] link that opens the run directory in OS file manager.
- All event types render identically to S-04 (same visual treatment).

---

### 5.6 Launch Sequence Modal (S-06)

**Purpose**: Two-step launcher — select sequence, optionally attach a context file, then confirm.

**Entry**: "Launch Sequence" button in S-03, [Run] on a sequence row, or ⚡ on card with prior run.

**Layout**: Centered modal, max-width 560px, backdrop overlay dims the screen behind.

**Step 1 — Sequence selection**:

- Modal title: "Launch Sequence"
- Sub-label: "Target project: [project name]" (read-only)
- Sequence list: scrollable, each row is a radio-select item showing sequence name + description (same description logic as S-03: first non-heading paragraph from .md file, resolves GAP-06). Clicking a row selects it.
- If a sequence was pre-filled (from ⚡ or [Run] button): that row is pre-selected, focus is on it.
- Empty state for list: "No sequences available. Add sequence files to [config dir]."
- Selected sequence row: `--primary-dim` background, `--primary` left border.

**Step 2 — Optional context file**:

Below the sequence list, always visible (not a separate step screen — single modal):

- Section label: "Attach context file (optional)"
- [Attach .md file] button (secondary): opens OS file picker filtered to `*.md` files. On selection, shows chip: "[filename.md] ×" where × removes the attachment.
- If file attached: chip visible with file name. Tooltip on chip shows full path.
- Constraint: only `.md` files accepted (file picker filter enforced; if OS bypassed, show inline error "Only .md files are supported.").

**Footer**:
- [Cancel] — closes modal, no action.
- [Launch] (primary) — disabled until a sequence is selected; enabled as soon as selection made.

**Interactions**:
- Clicking [Launch]: modal closes, S-04 Live Run View opens immediately. "Starting..." state shown while child process initializes.
- Escape key: closes modal (same as Cancel).
- Clicking backdrop: closes modal (same as Cancel).
- If launch fails immediately (e.g., CLI not found at this point): modal reopens with an error banner at top: "Failed to start: [error message]."

---

### 5.7 Settings Screen (S-07)

**Purpose**: Configure app-wide preferences (FR-6.1, FR-6.2).

**Entry**: Gear icon in Dashboard top bar.

**Layout**: Full-screen or large side-panel (designer preference: full-screen for clarity at this scope). Back button returns to Dashboard.

```
[Back "<- Dashboard" | "Settings" heading]
[Settings form, single column, labeled sections]
[Save button (sticky bottom or top-right)]
```

**Sections and Fields**:

**Section: Claude CLI**
- Field: "Claude CLI path" — text input. Placeholder: "Leave blank to use PATH". Inline [Browse...] button. Below field: resolved path shown in small text ("Currently using: /usr/local/bin/claude v1.2.3" or error if unresolvable).
- Field button: [Verify] — same verification flow as S-01.

**Section: Git Polling**
- Field: "Poll interval" — number input (seconds), min 5, max 3600. Default 10. Inline unit label "seconds". Validation: must be integer, in range.

**Section: Run Retention (per-project defaults)**
- Field: "Keep runs newer than" — number input (days), min 1. Default 30. Label "days".
- Field: "Max storage per project" — number input (MB), min 50. Default 500. Label "MB".
- Helper text: "Runs are pruned at startup and once daily. Both limits apply; whichever is exceeded first triggers pruning."

**Section: Display**
- Field: "Dashboard view" — radio group: "Grid" / "List". Synced with toolbar toggle.

**Save behavior**:
- [Save] button (primary): validates all fields, shows inline field errors if invalid, saves to config on success, shows a brief "Saved" confirmation next to the button (2 seconds, then fades).
- Unsaved changes: if user clicks Back with unsaved changes, inline prompt appears: "You have unsaved changes. Discard?" [Discard] [Keep editing].

---

### 5.8 Tag Editor Popover (S-08)

**Purpose**: Manage custom tags for a project card. (Resolves GAP-04.)

**Entry**: Right-click project card → "Edit Tags..." from context menu.

**Layout**: Small popover anchored to the project card, appearing near the right-click point. Max-width 280px.

```
[Popover header: "Edit Tags for [project name]"]
[Existing tags: chips with × to remove each]
[Add tag input + [Add] button]
[Footer: [Done] button]
```

**Components**:

- Existing tag chips: each shows the tag name + a small × icon. Clicking × removes the tag immediately (no separate save).
- Add tag input: text input, placeholder "New tag...". Character limit: 32 chars. [Add] button (secondary, small) or press Enter.
  - Validation: tag names trimmed, lowercased, no duplicates (if duplicate, input shakes and shows "Already added"). Empty input disables [Add] button.
  - On add: new chip appears instantly; input clears and refocuses.
- [Done] button (primary, small): closes popover. Changes are applied live (no cancel/discard flow needed since changes are granular and immediately applied).
- Clicking outside the popover: closes popover (same as Done).
- Escape key: closes popover.
- Tag chips on the card update in real time as tags are added/removed (popover is open while card is visible).

**Empty state** (no tags yet): "No tags. Add one below." shown in place of chip area.

---

### 5.9 Toast Notification System (S-09)

**Purpose**: In-app feedback for run terminal events (FR-5.1 through FR-5.4). No OS-native notifications in v1 (resolves GAP-09).

**Entry**: Automatically when a run reaches Completed, Failed, or Stopped state.

**Layout**: Fixed overlay, bottom-right corner of the window, stacked upward. Max 4 toasts visible simultaneously; older ones pushed up and fade out.

**Toast anatomy**:
```
[Outcome icon | "Sequence name" on "Project name" | status text | [×] dismiss]
```

- Outcome icon: colored per status (green check, red X, gray stop).
- Status text: "Completed", "Failed", "Stopped".
- [×] button: dismisses immediately.
- Entire toast is clickable (except ×): navigates to the relevant run view (S-05 for completed/stopped, S-04 if somehow still accessible).

**Auto-dismiss**:
- Success (Completed): auto-dismiss after 8 seconds. Progress bar along bottom of toast depletes over 8 seconds as visual timer.
- Failed: no auto-dismiss. Stays until user manually dismisses.
- Stopped: auto-dismiss after 8 seconds (same as success).

**Multiple toasts**: Each toast is independent. Failed toasts accumulate until dismissed.

**Animation**: Toasts slide in from the right on appear, fade + slide out on dismiss.

---

## 6. Component Library

Summary of reusable components referenced across screens.

| Component | Used In | Key States |
|---|---|---|
| ProjectCard | S-02 | default, hover, active-run, missing, loading (skeleton) |
| GitStatusBadge | S-02, S-03 | clean, dirty, ahead, behind, combined, loading, error |
| RunOutcomeBadge | S-02, S-03, S-04, S-05 | pending, running, completed, failed, stopped, never |
| EventBlock | S-04, S-05 | assistant, thinking, tool-call, tool-result, file-edit, user-input, system |
| SequenceRow | S-03, S-06 | default, selected, hover |
| TagChip | S-02, S-08 | default, removable, filter-active |
| Toast | S-09 | success, failed, stopped |
| RateLimitPill | S-02 | loaded, loading, unavailable |
| ContextMenu | S-02 | default (4 items + separator + conditional Relocate) |
| FileAttachChip | S-06 | attached, none |
| DiffBlock | S-04, S-05 | added lines (green), removed lines (red), context lines |

---

## 7. Edge Cases and States

| Scenario | Requirement | UI Handling |
|---|---|---|
| Project directory deleted after registration | FR-1.5 | Card enters "missing" state (gray edge, struck name, Relocate/Remove options) |
| CLI disappears mid-session | FR-3.2 | Non-dismissable top bar banner with link to S-01 |
| Run still active when app is re-launched | FR-3.5 | Startup: orphan detection kills stale child processes; run marked "failed" with note "Terminated (app restarted)" |
| Two runs active simultaneously on same project | FR-2.5 | Both show as "Running" rows in S-03; each has own S-04 view; card badge shows count "2 running" |
| Transcript JSONL missing or corrupt | FR-4.1, FR-4.2 | S-05 shows error state with path and [Open folder] link |
| Storage limit exceeded | FR-4.4 | Pruning is silent background operation; no UI disruption |
| No sequences defined | FR-2.2 | S-03 sequences panel and S-06 list show empty states with config dir path |
| Search yields no results | FR — | "No projects match your search." in grid area; clear search link |
| Tag filter yields no results | FR — | "No projects match the selected tags." with [Clear filters] link |
| Very long project name | FR-1.3 | Truncated with ellipsis; full name on hover tooltip |
| Very long path | FR-1.3 | Truncated with ellipsis; full path on hover tooltip |
| User sends input during run (GAP-02 resolved) | FR implied | UserInputBox always active; input sent to stdin; appears as user event in stream |
| `claude /usage` subprocess fails | GAP-01 resolved | Rate limit pill shows "--" with tooltip "Usage data unavailable"; no error blocking UI |
| $EDITOR not set | GAP-08 resolved | Falls back to OS default file association for directory; if OS association also fails, shows brief error toast "Could not open editor" |

---

## 8. Gaps

All gaps identified in the original spec have been resolved.

| ID | Original Question | Resolution |
|---|---|---|
| GAP-01 | Source and format of rate limit data | `claude /usage` run as subprocess; stdout parsed. Pill shows parsed values; popover shows full output. |
| GAP-02 | Is UserInputBox conditional on CLI prompt detection? | Always active during run; protocol deferred to architect. |
| GAP-03 | ⚡ quick-run with no prior run — what happens? | Opens S-03 Project Detail with Sequences panel focused and highlighted. |
| GAP-04 | Custom tag management entry point and interaction | Right-click context menu → "Edit Tags..." → S-08 inline popover on card. |
| GAP-05 | Running process detection mechanism | Deferred to architect. |
| GAP-06 | Sequence description source | First non-heading paragraph from sequence `.md` file; fallback "(No description)". |
| GAP-07 | Single parent directory or multiple root dirs? | Single parent directory confirmed. |
| GAP-08 | "Open in Editor" and "Open in Terminal" implementation | $EDITOR env var (editor); OS default file association fallback. OS default terminal (terminal). No additional settings. |
| GAP-09 | OS-native notifications? | Not in v1. In-app toasts only (FR-5.4 confirmed). |
| GAP-10 | Transcript file location and format | `<project>/.claude/runs/<run-id>/transcript.jsonl` JSONL. Confirmed. |

**No open gaps remain. This document is complete and ready for implementation handoff.**

---

## Appendix: Requirement Coverage Checklist

| Requirement | Covered By |
|---|---|
| FR-1.1 Register project | S-02 "+ Add Project", OS directory picker |
| FR-1.2 Remove project | S-02 right-click → Remove → inline confirm |
| FR-1.3 Project card display | S-02 ProjectCard component |
| FR-1.4 Git polling (10s, on focus, pause hidden) | S-02 card git badges; S-03 git status bar (behavior is runtime, UI reflects it) |
| FR-1.5 Missing project state | S-02 ProjectCard missing state |
| FR-2.1 Sequences as named config files | S-03 Sequences panel, S-06 sequence list |
| FR-2.2 Browse sequences | S-03 Sequences panel |
| FR-2.3 Launch sequence (project + sequence) | S-06 Launch Modal |
| FR-2.4 Attach .md file | S-06 file attach section |
| FR-2.5 No concurrency cap | S-02 "2 running" badge, S-03 multiple running rows |
| FR-3.1 Spawn CLI subprocess | Runtime; UI: S-04 live run |
| FR-3.2 CLI not found | S-01 Setup Screen |
| FR-3.3 Parsed rendering (tool calls, diffs, thinking) | S-04, S-05 EventBlock components |
| FR-3.4 Stop control | S-04 Stop button with inline confirm |
| FR-3.5 Crash / orphan detection | S-02: run marked failed on restart |
| FR-3.6 Run state transitions | RunOutcomeBadge covers all states |
| FR-4.1 Transcript persisted to project dir | S-05 reads from `<project>/.claude/runs/<run-id>/` |
| FR-4.2 meta.json, transcript.jsonl, raw.log | Runtime; S-05 error state references path |
| FR-4.3 Browse past runs, sorted newest-first | S-03 Run History panel |
| FR-4.4 Retention config | S-07 Settings retention section |
| FR-5.1 Toast on terminal state | S-09 Toast System |
| FR-5.2 Toast click opens run | S-09 toast click navigation |
| FR-5.3 Toast auto-dismiss (8s success, persist failed) | S-09 auto-dismiss rules |
| FR-5.4 No OS notifications | Confirmed; not designed |
| FR-6.1 Settings fields (poll, retention, CLI path) | S-07 all fields |
| FR-6.2 Settings persisted to OS config | Runtime; S-07 Save |
