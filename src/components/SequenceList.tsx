// SequenceList — fetches and renders the sequences panel for a project.
// Used in Project Detail (S-03). Empty/loading/error states per ui-ux-spec.md §5.3/§5.6.
// See docs/tasks/T3.2.md.
import React from "react";
import { useSequences } from "../hooks/useSequences";
import type { Sequence } from "../ipc/bindings";
import { SequenceRow } from "./SequenceRow";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface SequenceListProps {
  projectId: string;
}

// ---------------------------------------------------------------------------
// SequenceList
// ---------------------------------------------------------------------------

export function SequenceList({ projectId }: SequenceListProps) {
  const { data: sequences, isLoading, error } = useSequences(projectId);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);

  // FIX-5: reset selection when projectId changes. The effect body doesn't read
  // projectId, but the dependency is the intended trigger (reset on change).
  // biome-ignore lint/correctness/useExhaustiveDependencies: projectId is the intentional reset trigger
  React.useEffect(() => {
    setSelectedId(null);
  }, [projectId]);

  // FIX-4: wrap handlers in useCallback to avoid unnecessary re-renders of memoized rows
  const handleRun = React.useCallback((seq: Sequence) => {
    // Epic 4 will implement RunManager; stub only.
    console.log("Run sequence:", seq.name);
  }, []);

  const handleSelect = React.useCallback((seq: Sequence) => {
    setSelectedId(seq.name);
  }, []);

  if (isLoading) {
    return (
      <div style={styles.stateContainer}>
        <span style={styles.stateText}>Loading sequences...</span>
      </div>
    );
  }

  if (error) {
    const message = error instanceof Error ? error.message : String(error);
    return (
      <div style={styles.stateContainer}>
        <span style={styles.errorText}>{message}</span>
      </div>
    );
  }

  if (!sequences || sequences.length === 0) {
    return (
      <div style={styles.stateContainer}>
        {/* FIX-1: include directory hint per UI spec §5.3 */}
        <span style={styles.stateText}>
          No sequences found. Add .md files to .claude/sequences/ in your project.
        </span>
      </div>
    );
  }

  return (
    <ul style={styles.list}>
      {sequences.map((seq) => (
        <SequenceRow
          key={seq.name}
          sequence={seq}
          selected={selectedId === seq.name}
          onRun={handleRun}
          onSelect={handleSelect}
        />
      ))}
    </ul>
  );
}

export default SequenceList;

// ---------------------------------------------------------------------------
// Inline styles
// ---------------------------------------------------------------------------

const styles: Record<string, React.CSSProperties> = {
  list: {
    listStyle: "none",
    margin: 0,
    padding: 0,
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-1)",
  },
  stateContainer: {
    padding: "var(--space-4)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  stateText: {
    fontSize: "var(--font-size-sm)",
    color: "var(--text-secondary)",
  },
  errorText: {
    fontSize: "var(--font-size-sm)",
    color: "var(--error)",
  },
};
