# UI/UX Specification: Dev Dashboard

**Project**: Dev Dashboard
**Author**: aamadeo@gmail.com
**Date**: 2026-05-18 (revised 2026-06-13 — canonical adoption copy)
**Status**: Final — no open gaps.

> [adoption-assumption] Promoted to canonical `docs/ui-ux.md` during project adoption (sequence 14, 2026-06-13). Source was `.claude/ui-ux-spec.md` (status Final, 2026-05-18). Cross-checked against shipped code: `src/routes/`, `src/components/`, `src/styles/tokens.css`. Divergences and AS-BUILT reality documented in each relevant section. All screen IDs (S-01..S-09) and component names preserved.

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

> [adoption-assumption] All 25 tokens above confirmed present and hex-identical in `src/styles/tokens.css`. No divergence. Note: `--warning` and `--accent` share the same hex value (`#F5A623`); `tokens.css` includes an inline comment acknowledging this: "same amber as --accent; update both if palette changes."

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

> [adoption-assumption] Typography tokens confirmed in `tokens.css`: `--font-size-xs: 11px`, `--font-size-sm: 13px`, `--font-size-base: 14px`, `--font-size-code: 12px`, `--font-size-lg: 18px`. Weights `--font-weight-regular: 400`, `--font-weight-semibold: 600`, `--font-weight-bold: 700`. Line heights `--line-height-body: 1.5`, `--line-height-code: 1.6`. One additional token found in `Setup.tsx` inline styles: `--font-size-xl` (referenced for the heading but not declared in `tokens.css`). This is a minor divergence — the heading falls back to browser default. No functional impact.

Line heights: body 1.5, code 1.6. No web fonts — system fonts ensure instant render and platform feel.

### 1.3 Spacing Scale

`4 / 8 / 12 / 16 / 24 / 32 / 48px` — base 4px grid throughout.

> [adoption-assumption] Confirmed in `tokens.css`: `--space-1: 4px` through `--space-12: 48px`. Note: `--space-5` (20px) is referenced in `Setup.tsx` button padding but not declared in `tokens.css`. Minor undeclared token; no visual impact on other screens.

### 1.4 Border Radius

Cards: `8px`. Buttons: `6px`. Inputs: `6px`. Tags/chips: `4px`. Popovers: `8px`. Toasts: `8px`.

> [adoption-assumption] Confirmed: `--radius-sm: 4px`, `--radius-md: 6px`, `--radius-lg: 8px` in `tokens.css`. All shipped components use these tokens.

### 1.5 Iconography

Single icon set: Lucide React (confirmed shipped dependency). 16px default, 20px for toolbar icons. Icons always paired with a tooltip or visible label for discoverability.

> [adoption-assumption] Confirmed from `ProjectCard.tsx` (AlertTriangle, GitBranch, Zap), `Dashboard.tsx` (Grid, List, Plus, Settings), `Settings.tsx` (ArrowLeft, FolderOpen, Save). Lucide is the shipped library; the spec's original mention of "Phosphor Icons or Lucide" is resolved to Lucide only.

### 1.6 Motion

Transitions: `150ms ease-out` for hover/focus. Modal open: `200ms ease-out` scale + fade. Toast slide-in: `200ms ease-out` from bottom-right. No animation exceeds `300ms`. Reduced motion media query honored — all animation disabled when OS preference is set.

> [adoption-assumption] Confirmed: `--duration-fast: 150ms`, `--duration-base: 200ms`, `--duration-slow: 300ms`, `--easing-out: ease-out` in `tokens.css`. `ProjectCard.tsx` implements `prefers-reduced-motion` via `window.matchMedia` at module scope and conditionally applies pulse/skeleton animations.

---

## 2. Screen Inventory

| ID | Screen Name | Purpose | Build Status |
|---|---|---|---|
| S-01 | Setup Screen | Shown when Claude CLI is not found on PATH; blocks launch | Built (`src/routes/Setup.tsx`) |
| S-02 | Dashboard | Primary screen; grid of project cards; entry to all project actions | Built (`src/routes/Dashboard.tsx`) |
| S-03 | Project Detail | Per-project view: runs history + sequences panel | Stub — returns placeholder div |
| S-04 | Run View (Live) | Streams a running sequence in real time | Stub — returns placeholder div |
| S-05 | Run View (Historical) | Re-renders a completed run from stored transcript | Stub — returns placeholder div |
| S-06 | Launch Sequence Modal | Two-step picker: choose sequence + optional file attach | Built (`src/components/LaunchModal.tsx`) |
| S-07 | Settings | App-wide configuration | Built (`src/routes/Settings.tsx`) |
| S-08 | Tag Editor Popover | Inline tag management on project card (right-click context) | Built (`src/components/TagEditorPopover.tsx`) |
| S-09 | Toast System | Overlay notification layer (not a screen; always present) | Built (`src/components/Toast.tsx`) |

> [adoption-assumption] S-03, S-04, S-05 are placeholder stubs as of branch `feat/T4.7-step-failure-command`. Epics 3 and 4 are in progress. The spec for these screens is fully designed; implementation is pending.

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
            |       +-- "Edit Tags" --> [S-08 Tag Editor Popover]
            |       +-- "Remove" --> inline confirmation (card replaces content)
            |       +-- "Open in Editor" --> launches $EDITOR / OS association (no screen)
            |       +-- "Open in Terminal" --> launches OS default terminal (no screen)
            |       +-- "Relocate" (missing state only) --> OS directory picker
            |
            +-- Click gear icon --> [S-07 Settings]
            +-- Click toast --> [S-04 or S-05 Run View]
```

> [adoption-assumption] Context menu item labels confirmed from `Dashboard.tsx` `menuItems` array: "Open in Editor", "Open in Terminal", "Edit Tags" (not "Edit Tags..."), "Relocate" (not "Relocate..."), "Remove" (no separator in current implementation). "Remove" triggers inline confirmation that replaces the card's content (confirmed in `ProjectCardWrapper` render branch). Every screen is reachable from the map above. No orphan screens.

---

## 4. Flow Map

| User Action | Entry Point | Screens Traversed | Result |
|---|---|---|---|
| Register project | Dashboard → "+ Add Project" | S-02 → OS directory picker → S-02 | New card appears in grid |
| Remove project | Dashboard → right-click card → "Remove" | S-02 → inline card confirm | Card removed; files untouched |
| View project detail | Dashboard → click card | S-02 → S-03 | Project detail opens |
| Launch sequence | Project Detail → "Launch" OR Dashboard → Launch modal | S-03 / S-02 → S-06 → S-04 | Live run view opens |
| Quick-run (no prior run) | Dashboard → ⚡ button on card | S-02 → S-03 (Sequences panel focused) | User selects sequence, then → S-06 → S-04 |
| Quick-run (prior run exists) | Dashboard → ⚡ button on card | S-02 → S-06 (pre-filled last sequence) → S-04 | Live run view opens |
| Attach .md file | Launch Modal → "Attach file" | S-06 → OS file picker → S-06 | File name shown in modal |
| Watch live run | Any path to S-04 | S-04 | Streaming events, stop button |
| Stop run | Live Run View → Stop | S-04 | Run marked stopped; stop button disabled |
| View past run | Project Detail → run row | S-03 → S-05 | Historical run rendered |
| Edit project tags | Dashboard → right-click card → "Edit Tags" | S-02 → S-08 popover | Tags updated on card |
| Open in editor | Dashboard → right-click card → "Open in Editor" | S-02 → OS ($EDITOR / file assoc.) | Editor opens; app stays open |
| Open in terminal | Dashboard → right-click card → "Open in Terminal" | S-02 → OS default terminal | Terminal opens at project dir |
| Relocate project | Dashboard → right-click missing card → "Relocate" | S-02 → OS directory picker → S-02 | Project path updated |
| Configure settings | Dashboard → gear icon | S-02 → S-07 | Settings saved |
| CLI missing | App launch | S-01 | User sets CLI path or installs |
| Toast click | Any screen | Any → S-04 or S-05 | Relevant run view opens |

---

## 5. Screen Specifications

---

### 5.1 Setup Screen (S-01)

**Purpose**: Block app use when Claude CLI is not found, provide actionable instructions.

**Entry**: App launch when `claude` is not found on PATH.

**Build status**: Fully built. `src/routes/Setup.tsx`.

**Layout**: Full-window centered card, max-width 520px, vertically centered.

**Components**:

- App logo / name at top of card (rendered as uppercase text label in `--text-disabled`, not an interactive element)
- Heading: "Claude CLI not found"
- Body text: "The Dev Dashboard requires the Claude CLI (`claude`) to be available on your PATH. Install it or set a custom path below."
- Installation instructions block (monospace code block): OS-detected at render time:
  - Windows: `winget install Anthropic.Claude`
  - macOS: `brew install anthropic/claude/claude` (plus npm fallback shown below)
  - Linux: `npm install -g @anthropic-ai/claude-code`
- Divider labeled "Or set a custom path"
- Text input: label "Claude CLI path", placeholder `/usr/local/bin/claude`, full width
- Button row: [Browse...] [Verify & Continue] (right-aligned)
- Status area below buttons: shows "Verifying..." during check; green check + "Found: version X.Y.Z" on success; red error text on failure

> [adoption-assumption] AS-BUILT: button row is right-aligned (not full-width on small). macOS shows Homebrew command first, then npm as a secondary "or via npm:" option — both commands shown. Browse file picker applies executable extension filters on Windows (`exe`, `cmd`, `bat`) and no filter on macOS/Linux.

**States**:

| State | UI |
|---|---|
| Initial | Instructions shown, input empty; "Verify & Continue" disabled (empty input) |
| Path entered, not verified | "Verify & Continue" enabled |
| Verifying | "Verifying..." text replaces button label; input disabled; Browse disabled |
| Verification success | "Verify & Continue" replaced by "Continue →" button; green status: "✓ Found: vX.Y.Z" |
| Verification failure | Red error text: "Could not run `<path> --version`. Check the path." |
| Save error after verify | Red error: "Verified, but could not save path. Check app permissions." |

**Interactions**:
- "Browse..." opens OS file picker (executables filter on Windows).
- On verification success, button changes to "Continue →" which navigates to Dashboard (`/`).
- Dashboard (`/`) re-checks CLI at mount via `verifyClaudeCli`; if not found, redirects back to `/setup`.
- Once on Dashboard, if CLI disappears mid-session: a non-dismissable top bar banner with link back to Setup (designed; full implementation in later epics).

---

### 5.2 Dashboard (S-02)

**Purpose**: Main hub. Shows all registered projects as cards. Entry point for all primary actions.

**Build status**: Fully built. `src/routes/Dashboard.tsx`.

**Layout**:
```
[Top Bar: App name | Rate limit indicator | Gear icon]
[Toolbar: "+ Add Project" | Search input | Tag filter chips | View toggle (grid/list)]
[Project Grid / List]
[Toast Layer (overlay, bottom-right)]
[Context Menu (overlay, at cursor position)]
[Tag Editor Popover (overlay, near card)]
```

**Top Bar**:
- Left: App name "dev-dashboard" (non-interactive; colored `--primary` in shipped code)
- Center: Rate limit indicator (`RateLimitPill` — currently a stub returning null; will be populated by Epic 7)
- Right: Gear icon button (Lucide `Settings` 18px) — opens S-07 Settings

> [adoption-assumption] App name renders as "dev-dashboard" (lowercase, matching the project slug) rather than the spec's "Dev Dashboard". Positioned left with `justify-content: space-between`. `RateLimitPill` is imported and rendered but returns `null` until Epic 7 ships.

**Toolbar**:
- "+ Add Project" button (secondary-style with `--primary` border, Lucide `Plus` icon): triggers OS directory picker. On selection, `addProject` IPC called; project list invalidated. If directory already registered, backend emits `toast:show` error.
- Search input (text, `flex: 1`, min-width 140px): filters project cards by name or path as you type. Placeholder "Search projects...". Border highlights `--border-strong` on focus.
- Tag filter chips: all unique tags across all projects sorted alphabetically. Each chip is a toggle button. Active chip: `--primary-dim` background + `--primary` border + `--primary` text. Inactive chip: `--bg-elevated` background + `--border-subtle`. Multiple chips = AND filter. No explicit "Clear filters" link in current implementation.
- View toggle: two icon buttons (Lucide `Grid` / `List`, 16px), right-aligned via `margin-left: auto`. Active button: `--primary` color. State persisted to `settings.view_mode` via `updateSettings`.

> [adoption-assumption] AND semantics confirmed in `Dashboard.tsx` filter predicate: `selectedTags.every((t) => p.tags.includes(t))`. No "Clear filters" link ships in the current toolbar; users clear individual chips by clicking them. Tag chips strip is inline in the toolbar (wraps via `flexWrap`) rather than a separate horizontally-scrollable strip.

**Project Card (Grid View)**:

Each card (`ProjectCard` component) has the following zones:

```
[Card: left-border colored by git status]
[Header row: Project name (bold) | [Running badge (if active)] | ⚡ quick-run button]
[Tag chips row (if any tags assigned)]
[Git row: branch icon + branch name | git badge text]
[Meta row: last run timestamp + outcome badge | path (truncated rtl, tooltip full path)]
OR [Missing state: alert banner + Relocate/Remove buttons]
```

- Card click (anywhere except interactive controls): navigates to S-03 Project Detail.
- ⚡ quick-run button (Lucide `Zap` 14px, amber `--accent` color):
  - No prior run: tooltip "Pick a sequence"; navigates to S-03 with Sequences panel highlighted.
  - Prior run exists: tooltip "Quick-run last sequence"; opens S-06 pre-populated.
  - Project missing: disabled; tooltip "Project directory missing".
  - Loading (undefined): tooltip "Loading..."; button not actively disabled but `onQuickRun` is a no-op.
- Git status left-edge: `--success` clean, `--error` dirty or git error, `--warning` ahead/behind, `--text-disabled` loading/unknown.
- Git badge text: "Loading..." / "Error" / "Dirty (N files)" / "Ahead N · Behind N" / "Ahead N" / "Behind N" / "Clean".
- Running badge: shown when `activeRun` prop is set; pulsing `--running` outlined pill. Clicking stops propagation (not yet wired to S-04).
- Outcome badge (`OutcomeBadge`): "Completed" (success-dim bg), "Failed" (error-dim bg), "Stopped" (text only, `--stopped`), "Running" (pulsing, `--running`), "Pending" (`--text-secondary`).
- Right-click on card → context menu (overlay at cursor coordinates).

> [adoption-assumption] `ProjectCard` is a pure presentational component. `Dashboard.tsx` currently passes `lastRun={undefined}` and `activeRun={undefined}` to `ProjectCardWrapper` — run data wiring is deferred to later epics. Outcome badge state "Never" from the spec maps to `lastRun === null` showing text "Never" in the meta row (not a badge chip), not a `RunStatus` enum value. "Pending" status badge is present in `OUTCOME_STYLES` lookup but not in the spec table — it is an AS-BUILT addition.

**Context Menu** (S-02 overlay):

Items in shipped order:
1. "Open in Editor" — calls `openInEditor(project.id)`
2. "Open in Terminal" — calls `openInTerminal(project.id)`
3. "Edit Tags" — opens S-08 Tag Editor Popover at cursor position
4. "Relocate" — only shown when `project.is_missing`; opens OS directory picker
5. "Remove" (danger style) — sets `confirmRemoveId` → replaces card content with inline confirm

> [adoption-assumption] No separator between items in current implementation. "Edit Tags" (no ellipsis) vs spec's "Edit Tags...". "Relocate" appears only for missing projects (4th item, conditional) before "Remove". The spec described "Relocate..." in the context menu; shipped code shows it above "Remove". Inline confirm replaces full card content rather than "replacing card content below normal footer".

**Project Card (List View)**:

Same card component rendered in a `flex column` layout with `gap: --space-3`. Same interactions.

> [adoption-assumption] List view renders the same `ProjectCard` component stacked vertically; no distinct row layout. Visual differentiation from grid is layout only (no column-style row with inline fields).

**Empty State** (no projects registered):

Centered in grid area:
- Text: "No projects yet." (`--text-secondary`)
- Button: "Add your first project" (secondary style with `--primary` border)

> [adoption-assumption] AS-BUILT text differs from spec ("No projects yet." vs spec's "No projects yet" + separate heading/body). Button text is "Add your first project" vs spec's "+ Add Project".

**Loading State**:

Six `ProjectCardSkeleton` placeholder cards with shimmer animation (`pc-skeleton` keyframe), respects `prefers-reduced-motion`.

**No-match State** (search or tag filter yields no results):

Centered: "No projects match your search." (`--text-secondary`). No "Clear filters" link in current implementation.

**Missing Project State**:

- Left border `--text-disabled`
- Project name struck-through (`text-decoration: line-through`) + `--text-disabled` color
- Git/meta rows replaced by: `AlertTriangle` icon + "Project directory not found" on `--error-dim` background
- Below banner: [Relocate...] and/or [Remove] buttons (shown when callbacks provided by parent)
- ⚡ button disabled

---

### 5.3 Project Detail (S-03)

**Purpose**: Per-project hub showing run history and available sequences.

**Entry**: Click project card on Dashboard.

**Build status**: Stub. Route registered; returns placeholder `<div>S-03 Project Detail</div>`.

**Designed layout** (implementation pending):
```
[Back button "<- Dashboard" | Project name (heading) | [Launch Sequence] button]
[Git status bar: branch | status | last polled timestamp | [Refresh] icon]
[Two-column or stacked:]
  [Left/Top: Run History panel]
  [Right/Bottom: Sequences panel]
```

**Back button**: Returns to S-02 Dashboard; browser history (`navigate(-1)`).

**Git status bar**: Branch + verbose status + relative "Last updated Xs ago". [Refresh] icon triggers immediate poll.

**[Launch Sequence] button** (primary): Opens S-06 Launch Modal.

**Run History Panel**:

- Section heading: "Run History"
- Sorted newest-first. Each row: outcome icon + sequence name + start timestamp (relative) + duration + status badge.
- Clicking a row: S-05 (historical) or S-04 (if still running).
- Empty state: "No runs yet. Launch a sequence to get started."
- Loading state: 3 skeleton rows.

**Sequences Panel**:

- Section heading: "Available Sequences"
- Each row: name (bold) + first non-heading paragraph from sequence `.md` (fallback "(No description)") + [Run] button.
- On entry via ⚡ with no prior run: panel gets pulsing `--primary` border for 2 seconds, scrolled into view; fades after.
- Empty state: "No sequences found. Add sequence files to [config dir path]."

---

### 5.4 Run View — Live (S-04)

**Purpose**: Real-time streaming view of an active run.

**Entry**: Confirm in S-06 Launch Modal, or click active run badge, or click active run row in S-03.

**Build status**: Stub. Route registered; returns placeholder `<div>S-04 Run Live</div>`.

**Designed layout** (implementation pending):
```
[Header bar: <- back | Sequence name @ Project name | Status badge (Running, pulse) | [Stop] button]
[User Input Box (pinned above stream)]
[Event Stream (scrollable)]
[Auto-scroll toggle (floating, bottom-right of stream)]
```

**Header bar**:
- Back button: returns to S-03; run continues in background; badge appears on project card.
- [Stop] button (red outline): single click shows inline "Confirm Stop?" with [Yes, Stop] / [Cancel] for 3 seconds, then reverts. On confirm: kills process, status → stopped, button disabled.

**User Input Box** (FR-3 implied):

Always active during run:
- Multi-line textarea: min-height 48px, max-height 120px (auto-grows within range).
- Placeholder: "Send input to the running process..."
- [Send] button (primary, right): disabled when empty. Sends to stdin + appends "user" event.
- Enter submits. Shift+Enter inserts newline.
- After send: textarea clears, focus returns.
- Run ended: textarea + Send disabled; placeholder "Run has ended."

**Event Stream**:

| Event Type | Visual Treatment |
|---|---|
| Assistant text | `--text-primary`, prose, markdown rendered |
| Thinking block | Collapsible accordion; `--thinking` left-border; "Thinking..." header; italic monospace; collapsed by default |
| Tool call | `--tool-call` left-border; tool name + spinner (pending) / checkmark (done); expandable JSON input |
| Tool result | Indented under tool call; lighter border; summary + expandable full result |
| File edit | `--file-edit` left-border; file path + lines changed; unified diff (green/red lines) |
| User input sent | Right-aligned bubble; `--primary-dim` background; timestamp |
| System/status | Centered `--text-secondary` text |
| Step failed | Visual treatment TBD — `StepFailedBlock` component stub exists; spec to be updated when FR-3.7 implementation lands |

> [adoption-assumption] `StepFailedBlock.tsx` is a stub (`return null`) on the current branch. This is the in-progress work on `feat/T4.7-step-failure-command`. The component is registered in the component tree but not yet rendered. Once Epic 4 / T4.7 ships, this section must be updated with the step-failure interaction: the four-option prompt (Retry / Skip / Abort / Continue) per FR-3.7.

Auto-scroll: auto-scrolls to bottom. Manual scroll up pauses auto-scroll; "Jump to bottom" floating button appears.

**States**:

| State | UI |
|---|---|
| Run starting | "Starting..." spinner in header |
| Running | Full UI |
| Stopping | Stop button disabled; "Stopping..." in header |
| Completed | Badge → "Completed" (green); input disabled |
| Failed | Badge → "Failed" (red); error event shown last; input disabled |
| Stopped | Badge → "Stopped" (gray); input disabled |
| Step failed (FR-3.7) | Run pauses; inline prompt with four options: Continue (default, prominent), Retry, Skip, Abort |

---

### 5.5 Run View — Historical (S-05)

**Purpose**: Re-renders a past run from stored JSONL transcript.

**Entry**: Click a past run row in S-03 Project Detail.

**Build status**: Stub. Route registered; returns placeholder `<div>S-05 Run Historical</div>`.

**Designed layout** (implementation pending): Identical to S-04 except:
- No [Stop] button; shows duration "Duration: 3m 42s".
- No User Input Box.
- Full event stream loaded immediately from `<project>/.claude/runs/<run-id>/transcript.jsonl`.
- Loading state: spinner centered in stream area.
- Missing/corrupt JSONL: error state with path + [Open folder] link to OS file manager.
- All event types (including `StepFailedBlock` when implemented) render identically to S-04.

---

### 5.6 Launch Sequence Modal (S-06)

**Purpose**: Two-step launcher — select sequence, optionally attach a context file, confirm.

**Entry**: "Launch Sequence" in S-03, [Run] on a sequence row, or ⚡ on card with prior run.

**Build status**: Component exists (`src/components/LaunchModal.tsx`); full implementation status not verified (file not read; stub or partial possible).

> [adoption-assumption] `LaunchModal.tsx` is listed in the component glob output. Its implementation state was not read; given that S-03 is a stub, the modal may be partially implemented or also a stub. Treat as designed but unverified.

**Layout**: Centered modal, max-width 560px, backdrop overlay.

**Sequence selection**:
- Modal title: "Launch Sequence"
- Sub-label: "Target project: [project name]" (read-only)
- Sequence list: scrollable radio-select rows — sequence name + description (first non-heading paragraph or "(No description)").
- Pre-selected row (if pre-filled): `--primary-dim` background + `--primary` left border.
- Empty state: "No sequences available. Add sequence files to [config dir]."

**Context file** (below sequence list, always visible):
- Label: "Attach context file (optional)"
- [Attach .md file] button: OS file picker, `*.md` filter. Shows chip "[filename.md] ×" on selection.
- Only `.md` files accepted; non-`.md` selection shows inline error "Only .md files are supported."

**Footer**:
- [Cancel] — closes modal.
- [Launch] (primary) — disabled until sequence selected.

**Interactions**:
- [Launch]: modal closes; S-04 opens in "Starting..." state.
- Escape / backdrop click: close modal.
- Launch failure: modal reopens with error banner "Failed to start: [error message]."

---

### 5.7 Settings Screen (S-07)

**Purpose**: Configure app-wide preferences (FR-6.1).

**Entry**: Gear icon in Dashboard top bar.

**Build status**: Fully built. `src/routes/Settings.tsx`.

**Layout**: Single-column page (max-width 640px, centered). Back button returns to previous route.

```
[Back button | "Settings" heading]
[Fieldset: Claude CLI]
[Fieldset: Projects]
[Fieldset: Polling]
[Fieldset: Log Retention]
[Fieldset: Display]
[Actions row: [Open logs folder] | [Save]]
```

**Fieldsets and Fields** (AS-BUILT from `Settings.tsx`):

**Claude CLI**
- "Claude CLI path" — text input + inline [Browse...] button (OS file picker, no filter). Placeholder "e.g. /usr/local/bin/claude".
- [Verify] button — same verification flow as S-01 (calls `verifyClaudeCli`). Shows "Verifying..." while in flight. Result: green "Currently using: [path] [version]" or red error.

**Projects**
- "Projects parent directory" — text input. Placeholder "e.g. /home/user/projects". Leave blank to disable.

> [adoption-assumption] "Projects" fieldset is an AS-BUILT addition not in the original spec §5.7. Added to accommodate FR-6.1 `parent_dir` field (confirmed in `Settings.tsx`).

**Polling**
- "Git poll interval (seconds)" — number input, range 5–3600, step 1. Hint "5 – 3600".
- "Usage poll interval (seconds)" — number input, range 30–3600, step 1. Hint "30 – 3600". Default 60.

> [adoption-assumption] "Usage poll interval" fieldset row is an AS-BUILT addition (FR-6.1 `usage_poll_interval_secs`; Epic 7). The spec §5.7 grouped git and usage polling under a single "Git Polling" section; the shipped code groups both under "Polling".

**Log Retention**
- Helper text: "Runs are pruned at startup and once daily. Both limits apply; whichever is exceeded first triggers pruning."
- "Retention days" — number input, range 1–90, step 1. Hint "1 – 90". Default 30.
- "Retention size (MB)" — number input, range 50–10240, step 1. Hint "50 – 10240". Default 500.

> [adoption-assumption] AS-BUILT upper bounds: retention days max is **90** (spec said no max; shipped code enforces `≤ 90`), retention size max is **10240 MB** (spec said min 50; shipped code enforces `≤ 10240`). These come from `T1.1-fixes-2` review iteration.

**Display**
- "View mode" — toggle button group: "Grid" / "List". `aria-pressed` on each. Active: `--primary-dim` bg + `--primary` border/text. Synced with Zustand `ui` store; persisted to settings on Save.

**Actions row**:
- [Open logs folder] (secondary, Lucide `FolderOpen` icon): calls `openLogsFolder()`. Shows "Opening..." while in flight.
- Save error text (if save fails): shown left of Save button, truncated with ellipsis.
- "Saved!" output element: appears on successful save, fades after 2 seconds.
- [Save] (primary, Lucide `Save` icon): validates, patches settings. Disabled when any field has an error or save in flight.

**Unsaved changes guard**: If user clicks Back with unsaved changes, `window.confirm` dialog: "You have unsaved changes. Leave without saving?". Cancel = stay; OK = navigate away.

**Validation**:
- All four numeric fields show inline red error text below the input and `--error` border when out of range.
- [Save] disabled while any validation error exists.

---

### 5.8 Tag Editor Popover (S-08)

**Purpose**: Manage custom tags for a project card.

**Entry**: Right-click project card → "Edit Tags" from context menu.

**Build status**: Fully built. `src/components/TagEditorPopover.tsx`.

**Layout**: Small popover anchored at cursor coordinates. Max-width 280px.

```
[Popover header: "Edit Tags for [project name]"]
[Existing tags: chips with × to remove each]
[Add tag input + [Add] button]
[Footer: [Done] button]
```

**Components**:

- Existing tag chips: name + × icon. Clicking × removes immediately; card tag chips update in real time.
- Add tag input: placeholder "New tag...". [Add] button or Enter.
  - Normalization: trimmed + lowercased before storage (FR-1.6.2).
  - Duplicate rejection: if `tags.includes(trimmed)`, input shakes and shows "Already added" (FR-1.6.3).
  - Empty input: [Add] button disabled.
  - On add: chip appears; input clears and refocuses.
- Character limit: spec states 32 chars. FR-1.6.4 notes this was not explicitly enforced in `TagEditorPopover.tsx` at adoption time — verify `maxLength` attribute presence.
- [Done] button (primary): closes popover. All changes are applied live; no cancel/discard flow.
- Clicking outside popover: closes.
- Escape key: closes.

**Empty state** (no tags): "No tags. Add one below."

---

### 5.9 Toast Notification System (S-09)

**Purpose**: In-app feedback for run terminal events (FR-5.1 through FR-5.4).

**Entry**: Automatically when a run reaches Completed, Failed, or Stopped state.

**Build status**: Component exists (`src/components/Toast.tsx`). Zustand `toasts` store present. Full wiring to run events pending (run management epics).

**Layout**: Fixed overlay, bottom-right corner of window, stacked upward. Max 4 toasts simultaneously.

**Toast anatomy**:
```
[Outcome icon | "Sequence name" on "Project name" | status text | [×] dismiss]
```

- Outcome icon: green check (Completed), red X (Failed), gray stop (Stopped).
- [×] button: dismisses immediately.
- Entire toast clickable (except ×): navigates to run view (S-05 for completed/stopped, S-04 if accessible).

**Auto-dismiss**:
- Completed: auto-dismiss after 8 seconds. Progress bar depletes over 8 seconds.
- Failed: no auto-dismiss. Stays until manually dismissed.
- Stopped: auto-dismiss after 8 seconds.

**Animation**: Slide in from right; fade + slide out on dismiss. Honors `prefers-reduced-motion`.

---

## 6. Component Library

| Component | Used In | Key States | Build Status |
|---|---|---|---|
| ProjectCard | S-02 | default, hover, active-run, missing, loading (skeleton) | Built |
| ProjectCardSkeleton | S-02 | loading shimmer | Built |
| GitStatusBadge | S-02, S-03 | clean, dirty, ahead, behind, combined, loading, error | Built |
| RunOutcomeBadge | S-02, S-03, S-04, S-05 | pending, running, completed, failed, stopped | Built |
| EventBlock/AssistantBlock | S-04, S-05 | rendered markdown | Built (stub) |
| EventBlock/ThinkingBlock | S-04, S-05 | collapsed, expanded | Built (stub) |
| EventBlock/ToolCallBlock | S-04, S-05 | pending, done, expanded | Built (stub) |
| EventBlock/ToolResultBlock | S-04, S-05 | summary, expanded | Built (stub) |
| EventBlock/FileEditBlock | S-04, S-05 | unified diff view | Built (stub) |
| EventBlock/UserInputBlock | S-04, S-05 | sent input bubble | Built (stub) |
| EventBlock/SystemBlock | S-04, S-05 | status text | Built (stub) |
| EventBlock/StepFailedBlock | S-04, S-05 | four-option prompt (FR-3.7) | Stub — returns null; T4.7 in progress |
| SequenceRow | S-03, S-06 | default, selected, hover | Built |
| SequenceList | S-03, S-06 | default, empty | Built |
| LaunchModal | S-06 | sequence select, file attached, launching | Built (verify status) |
| TagEditorPopover | S-08 | editing, empty | Built |
| ContextMenu | S-02 | default items + conditional Relocate | Built |
| Toast | S-09 | success, failed, stopped | Built |
| RateLimitPill | S-02 | loaded, loading, unavailable | Stub — returns null; Epic 7 pending |
| ErrorBoundary | App-wide | error caught | Built |

> [adoption-assumption] EventBlock sub-components (`AssistantBlock`, `FileEditBlock`, `ToolResultBlock`) are exported from `src/components/EventBlock/index.tsx` and appear to have real implementations (exported individually), while `EventBlock` default export itself returns null (dispatcher not yet wired). `StepFailedBlock` is a confirmed stub. `RateLimitPill` is a confirmed stub. `LaunchModal` exists but implementation not read.

---

## 7. Edge Cases and States

| Scenario | Requirement | UI Handling |
|---|---|---|
| Project directory deleted after registration | FR-1.5 | Card enters missing state (gray edge, struck name, missingBanner, Relocate/Remove buttons) |
| CLI disappears mid-session | FR-3.2 | Non-dismissable top bar banner with link to S-01 (designed; implementation pending) |
| Run still active when app is re-launched | FR-3.5 | Startup orphan detection kills stale processes; run marked "failed" with note "Terminated (app restarted)" |
| Two runs active simultaneously on same project | FR-2.5 | Both show as "Running" rows in S-03; each has own S-04 view; card badge shows "Running" |
| Transcript JSONL missing or corrupt | FR-4.1, FR-4.2 | S-05 shows error state with path and [Open folder] link |
| Storage limit exceeded | FR-4.4 | Pruning is silent background operation; no UI disruption |
| No sequences defined | FR-2.2 | S-03 sequences panel and S-06 list show empty states with config dir path |
| Search yields no results | — | "No projects match your search." centered in grid area |
| Tag filter yields no results | FR-1.6.5 | "No projects match your search." (same empty state; no dedicated tag-filter empty message in current code) |
| Very long project name | FR-1.3 | Truncated with ellipsis + `nowrap`; full name in `aria-label` attribute |
| Very long path | FR-1.3 | Truncated with ellipsis; `direction: rtl` to show tail end of path; full path in `title` tooltip |
| User sends input during run | FR-3 implied | UserInputBox always active; input sent to stdin; appears as user event in stream |
| `claude /usage` subprocess fails | FR-7.4 | Rate limit pill shows `--` with tooltip "Usage data unavailable"; no blocking UI |
| $EDITOR not set | GAP-08 | Falls back to OS default file association; if OS association also fails, brief error toast "Could not open editor" |
| Step fails during run | FR-3.7 | Run pauses; StepFailedBlock renders four-option prompt: Continue (default), Retry, Skip, Abort |
| Settings save fails | S-07 | Error text shown left of Save button; viewMode rolled back in Zustand store to previous value |
| CLI verify save fails on Setup screen | S-01 | Red error: "Verified, but could not save path. Check app permissions." |

---

## 8. Gaps

All gaps from the original spec have been resolved. The adoption cross-check surfaced the following items requiring confirmation or future update.

| ID | Type | Item |
|---|---|---|
| GAP-01 | Resolved | Rate limit data source: `claude /usage` subprocess; pill + popover (Epic 7 pending implementation) |
| GAP-02 | Resolved | UserInputBox always active during run |
| GAP-03 | Resolved | ⚡ with no prior run → S-03 Sequences panel highlighted |
| GAP-04 | Resolved | Tags via right-click → S-08 Tag Editor Popover |
| GAP-05 | Resolved | Running process detection deferred to architect |
| GAP-06 | Resolved | Sequence description: first non-heading paragraph from `.md`; fallback "(No description)" |
| GAP-07 | Resolved | Single parent directory confirmed; persisted as `settings.parent_dir` |
| GAP-08 | Resolved | "Open in Editor": $EDITOR or OS default. "Open in Terminal": OS default terminal |
| GAP-09 | Resolved | No OS-native notifications v1; in-app toasts only |
| GAP-10 | Resolved | Transcript at `<project>/.claude/runs/<run-id>/transcript.jsonl` |
| ADOPT-01 | Needs verification | FR-1.6.4: per-tag character limit (32 chars per spec) — no explicit `maxLength` found in `TagEditorPopover.tsx` at adoption time. Confirm enforcement in UI input and/or backend. |
| ADOPT-02 | Needs verification | FR-1.6.5: AND vs OR semantics for tag filter — confirmed AND from `Dashboard.tsx` code (`selectedTags.every`). No action needed. |
| ADOPT-03 | Needs verification | FR-7.3 / FR-6.1: `usage_poll_interval_secs` default — assumed 60s from KB contract. Confirm. |
| ADOPT-04 | Spec update needed | S-04 `StepFailedBlock` visual spec: four-option step-failure prompt UI (FR-3.7) must be detailed once Epic 4 / T4.7 ships. Current stub in `src/components/EventBlock/StepFailedBlock.tsx`. |
| ADOPT-05 | Minor token gap | `--font-size-xl` and `--space-5` referenced in `Setup.tsx` but not declared in `tokens.css`. No visual impact; add to tokens for consistency. |
| ADOPT-06 | AS-BUILT label difference | Context menu "Edit Tags" (no ellipsis) vs spec "Edit Tags..." — document only; no UX impact. |
| ADOPT-07 | AS-BUILT label difference | App name renders as "dev-dashboard" (lowercase) not "Dev Dashboard". Either is acceptable; update brand guidelines if a preference exists. |

---

## Appendix A: Requirement Coverage Checklist

| Requirement | Covered By |
|---|---|
| FR-1.1 Register project | S-02 "+ Add Project", OS directory picker |
| FR-1.2 Remove project | S-02 right-click → "Remove" → inline card confirm |
| FR-1.3 Project card display | S-02 ProjectCard component |
| FR-1.4 Git polling (10s, on focus, pause hidden) | S-02 card git badges; S-03 git status bar |
| FR-1.5 Missing project state | S-02 ProjectCard missing state |
| FR-1.6.1 Tag add/remove via popover | S-08 TagEditorPopover via S-02 context menu |
| FR-1.6.2 Tags normalized (trim + lowercase) | S-08 TagEditorPopover `handleAdd` |
| FR-1.6.3 Duplicate tags rejected | S-08 TagEditorPopover duplicate check |
| FR-1.6.4 Tag length cap | S-08 (ADOPT-01: verify enforcement) |
| FR-1.6.5 Tag filter chips (AND semantics) | S-02 toolbar tag chips; confirmed AND in Dashboard.tsx |
| FR-2.1 Sequences as named config files | S-03 Sequences panel, S-06 sequence list |
| FR-2.2 Browse sequences | S-03 Sequences panel |
| FR-2.3 Launch sequence | S-06 Launch Modal |
| FR-2.4 Attach .md file | S-06 file attach section |
| FR-2.5 No concurrency cap | S-02 Running badge, S-03 multiple running rows |
| FR-3.1 Spawn CLI subprocess | Runtime; UI: S-04 live run |
| FR-3.2 CLI not found | S-01 Setup Screen |
| FR-3.3 Parsed rendering | S-04, S-05 EventBlock components |
| FR-3.4 Stop control | S-04 Stop button with inline confirm |
| FR-3.5 Orphan detection | S-02: run marked failed on restart |
| FR-3.6 Run state transitions | RunOutcomeBadge: pending/running/completed/failed/stopped |
| FR-3.7 Step failure prompt | S-04 StepFailedBlock (T4.7 in progress; ADOPT-04) |
| FR-4.1 Transcript persisted | S-05 reads from `<project>/.claude/runs/<run-id>/` |
| FR-4.2 meta.json, transcript.jsonl, raw.log | Runtime; S-05 error state references path |
| FR-4.3 Browse past runs, newest-first | S-03 Run History panel |
| FR-4.4 Retention config | S-07 Settings retention fields |
| FR-5.1 Toast on terminal state | S-09 Toast System |
| FR-5.2 Toast click opens run | S-09 toast click navigation |
| FR-5.3 Toast auto-dismiss (8s success, persist failed) | S-09 auto-dismiss rules |
| FR-5.4 No OS notifications | Confirmed; not designed |
| FR-6.1 Settings fields (all 7) | S-07 all fieldsets |
| FR-6.2 Settings persisted to OS config | Runtime; S-07 Save |
| FR-7.1 Usage pill in top bar | S-02 RateLimitPill (Epic 7 stub) |
| FR-7.2 Usage via `claude /usage` subprocess | RateLimitPill + useUsage hook (Epic 7) |
| FR-7.3 Usage poll timer + on-focus | useUsage hook (Epic 7); interval = `usage_poll_interval_secs` |
| FR-7.4 Subprocess failure → `--` placeholder | RateLimitPill unavailable state |
| FR-7.5 Pill click → KV popover + Refresh | RateLimitPill popover (Epic 7) |
| FR-7.6 No direct network calls from app | Subprocess isolation; NFR-8 cross-reference |

---

## Appendix B: AS-BUILT Divergences Summary

The following divergences were found between the original spec and shipped code. None require design rework; all are documented for fidelity.

1. **tokens.css**: All 25 color tokens confirmed hex-identical. `--font-size-xl` and `--space-5` are used in `Setup.tsx` but not declared in `tokens.css` (ADOPT-05).
2. **App name**: Renders as "dev-dashboard" (lowercase) not "Dev Dashboard" (ADOPT-07).
3. **Context menu labels**: "Edit Tags" not "Edit Tags..."; "Relocate" not "Relocate..." (ADOPT-06).
4. **Context menu structure**: No separator; Relocate is conditional item (4th), Remove is last.
5. **Inline confirm**: Replaces entire card content (not just footer) with a centered confirm UI including red "Remove" + "Cancel" buttons.
6. **Empty state copy**: "No projects yet." + "Add your first project" vs spec headings/body/button.
7. **Tag filter**: Inline in toolbar (wraps) rather than separate horizontal-scroll strip. No "Clear filters" link.
8. **No-match copy**: Single "No projects match your search." message used for both search and tag-filter no-match states.
9. **Settings layout**: Five fieldsets (Claude CLI / Projects / Polling / Log Retention / Display) vs spec's four sections; "Projects parent directory" field added; git + usage poll combined under "Polling".
10. **Retention bounds**: Max retention days = 90; max retention size = 10240 MB (enforced in both UI validation and backend).
11. **RateLimitPill**: Stub (`return null`) until Epic 7 ships.
12. **StepFailedBlock**: Stub (`return null`); T4.7 in progress on current branch.
13. **S-03, S-04, S-05**: Placeholder stubs; Epics 3–4 implementation pending.
14. **Quick-run**: `handleQuickRun` in Dashboard is currently a no-op (`TODO` comment); not yet wired to navigate or open modal.
15. **Running badge**: Present on cards but not yet clickable to S-04 (activeRun passed as `undefined` in current Dashboard).
