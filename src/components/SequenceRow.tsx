// SequenceRow — renders a single sequence item with name, description, and Run button.
// Used in SequenceList (S-03 Sequences panel) and LaunchModal (S-06).
// See ui-ux-spec.md §5.3, §5.6 and docs/tasks/T3.2.md.
import React from "react";
import type { Sequence } from "../ipc/bindings";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface SequenceRowProps {
  sequence: Sequence;
  selected?: boolean;
  onRun: (sequence: Sequence) => void;
  onSelect?: (sequence: Sequence) => void;
}

// ---------------------------------------------------------------------------
// Module-level static style constants (FIX-6: hoisted from render body)
// ---------------------------------------------------------------------------

const CONTENT_STYLE: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
};

const NAME_STYLE: React.CSSProperties = {
  fontSize: "var(--font-size-base)",
  fontWeight: "var(--font-weight-semibold)",
  color: "var(--text-primary)",
};

const DESCRIPTION_STYLE: React.CSSProperties = {
  fontSize: "var(--font-size-sm)",
  color: "var(--text-secondary)",
  lineHeight: "var(--line-height-body)",
  wordBreak: "break-word",
};

const RUN_BTN_STYLE: React.CSSProperties = {
  flexShrink: 0,
  fontSize: "var(--font-size-sm)",
  fontWeight: "var(--font-weight-semibold)",
  color: "var(--text-primary)",
  background: "var(--bg-elevated)",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-md)",
  padding: "var(--space-1) var(--space-3)",
  cursor: "pointer",
  transition: "background var(--duration-fast) var(--easing-out), border-color var(--duration-fast) var(--easing-out)",
  alignSelf: "flex-start",
};

// ---------------------------------------------------------------------------
// SequenceRow — wrapped in React.memo (FIX-3)
// ---------------------------------------------------------------------------

export const SequenceRow = React.memo(function SequenceRow({ sequence, selected = false, onRun, onSelect }: SequenceRowProps) {
  const [hovered, setHovered] = React.useState(false);

  // rowStyle depends on selected and hovered state — kept inline (FIX-6)
  const rowStyle: React.CSSProperties = {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "var(--space-3)",
    padding: "var(--space-3) var(--space-4)",
    borderRadius: "var(--radius-md)",
    cursor: onSelect ? "pointer" : "default",
    transition: "background var(--duration-fast) var(--easing-out)",
    background: selected
      ? "var(--primary-dim)"
      : hovered
        ? "var(--bg-hover)"
        : "transparent",
    // FIX-2: left border only for selected state; transparent placeholder for layout stability
    borderLeft: selected ? "3px solid var(--primary)" : "3px solid transparent",
    userSelect: "none",
  };

  function handleRowClick() {
    onSelect?.(sequence);
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect?.(sequence);
    }
  }

  function handleRunClick(e: React.MouseEvent) {
    e.stopPropagation();
    onRun(sequence);
  }

  return (
    <div
      role="listitem"
      aria-selected={selected}
      data-selected={selected ? "true" : "false"}
      tabIndex={onSelect ? 0 : undefined}
      style={rowStyle}
      onClick={handleRowClick}
      onKeyDown={handleKeyDown}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div style={CONTENT_STYLE}>
        <span style={NAME_STYLE}>{sequence.name}</span>
        <span style={DESCRIPTION_STYLE}>{sequence.description}</span>
      </div>
      <button
        type="button"
        aria-label={`Run ${sequence.name}`}
        style={RUN_BTN_STYLE}
        onClick={handleRunClick}
      >
        Run
      </button>
    </div>
  );
});

export default SequenceRow;
