// ProjectCard — see ui-ux-spec.md §5.2 and docs/tasks/T2.5.md.
// Pure presentational component: no hooks, no invoke(), no Zustand reads.
import { AlertTriangle, GitBranch, Zap } from "lucide-react";
import React from "react";
import type { GitStatus, Project, Run, RunStatus } from "../ipc/bindings";

// ---------------------------------------------------------------------------
// Pulse animation — injected once into the document head
// ---------------------------------------------------------------------------

const PULSE_STYLE_ID = "projectcard-pulse-keyframes";

function ensurePulseKeyframes() {
  if (typeof document === "undefined") return;
  if (document.getElementById(PULSE_STYLE_ID)) return;
  const style = document.createElement("style");
  style.id = PULSE_STYLE_ID;
  style.textContent = `
    @keyframes pc-pulse {
      0%, 100% { opacity: 1; }
      50% { opacity: 0.4; }
    }
    @keyframes pc-skeleton {
      0%, 100% { opacity: 0.4; }
      50% { opacity: 0.8; }
    }
    @media (prefers-reduced-motion: reduce) {
      .pc-pulse, .pc-skeleton { animation: none !important; }
    }
  `;
  document.head.appendChild(style);
}
ensurePulseKeyframes();

// Detect reduced motion at module scope; updates on page reload.
// Guard for test environments (jsdom) that do not implement matchMedia.
const reducedMotion =
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const skeletonAnimation: React.CSSProperties = reducedMotion
  ? {}
  : { animation: "pc-skeleton 1.5s ease-in-out infinite" };

// ---------------------------------------------------------------------------
// Helper: format relative timestamp
// ---------------------------------------------------------------------------

function formatRelative(isoString: string): string {
  const date = new Date(isoString);
  const now = Date.now();
  const diffMs = now - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  if (diffSec < 60) return "just now";
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay}d ago`;
}

// ---------------------------------------------------------------------------
// Helper: derive git left-edge color from GitStatus
// ---------------------------------------------------------------------------

function gitEdgeColor(gitStatus: GitStatus | undefined): string {
  if (!gitStatus) return "var(--text-disabled)";
  if (gitStatus.error !== null) return "var(--error)";
  if (gitStatus.dirty_files > 0) return "var(--error)";
  if (gitStatus.ahead > 0 || gitStatus.behind > 0) return "var(--warning)";
  if (gitStatus.is_clean) return "var(--success)";
  return "var(--text-disabled)";
}

// ---------------------------------------------------------------------------
// Helper: derive git badge text
// ---------------------------------------------------------------------------

function gitBadgeText(gitStatus: GitStatus | undefined): string {
  if (!gitStatus) return "Loading...";
  if (gitStatus.error !== null) return "Error";
  if (gitStatus.dirty_files > 0) return `Dirty (${gitStatus.dirty_files} files)`;
  const ahead = gitStatus.ahead > 0;
  const behind = gitStatus.behind > 0;
  if (ahead && behind) return `Ahead ${gitStatus.ahead} · Behind ${gitStatus.behind}`;
  if (ahead) return `Ahead ${gitStatus.ahead}`;
  if (behind) return `Behind ${gitStatus.behind}`;
  return "Clean";
}

// ---------------------------------------------------------------------------
// Module-level outcome badge styles (lookup table, no per-render allocs)
// ---------------------------------------------------------------------------

const outcomeBase: React.CSSProperties = {
  fontSize: "var(--font-size-xs)",
  borderRadius: "var(--radius-sm)",
  padding: "1px 6px",
  // Correct cast — fontWeight accepts string | number
  fontWeight: "var(--font-weight-semibold)" as React.CSSProperties["fontWeight"],
  display: "inline-block",
};

// Apply reduced-motion guard to Running animation
const OUTCOME_STYLES: Record<RunStatus, React.CSSProperties> = {
  Completed: { ...outcomeBase, color: "var(--success)", background: "var(--success-dim)" },
  Failed: { ...outcomeBase, color: "var(--error)", background: "var(--error-dim)" },
  Stopped: { ...outcomeBase, color: "var(--stopped)" },
  Running: {
    ...outcomeBase,
    color: "var(--running)",
    ...(reducedMotion ? {} : { animation: "pc-pulse 1.5s ease-in-out infinite" }),
  },
  Pending: { ...outcomeBase, color: "var(--text-secondary)" },
};

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface ProjectCardProps {
  project: Project;
  gitStatus?: GitStatus;
  lastRun?: Run | null;
  activeRun?: Run | null;
  onQuickRun?: () => void;
  onCardClick?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  // Optional action callbacks shown only in missing state
  onRelocate?: () => void;
  onRemove?: () => void;
}

// ---------------------------------------------------------------------------
// ProjectCard (wrapped in React.memo to avoid re-renders on every poll)
// ---------------------------------------------------------------------------

export const ProjectCard = React.memo(function ProjectCard({
  project,
  gitStatus,
  lastRun,
  activeRun,
  onQuickRun,
  onCardClick,
  onContextMenu,
  onRelocate,
  onRemove,
}: ProjectCardProps) {
  const edgeColor = gitEdgeColor(gitStatus);

  // useMemo so the merged style object is only recreated when edgeColor changes
  const cardStyle = React.useMemo(
    () => ({ ...styles.card, borderLeftColor: edgeColor }),
    [edgeColor]
  );

  // undefined (loading) → "Loading...", null → "Pick a sequence", Run → "Quick-run last sequence"
  const quickRunTitle = project.is_missing
    ? "Project directory missing"
    : lastRun === null
      ? "Pick a sequence"
      : lastRun === undefined
        ? "Loading..."
        : "Quick-run last sequence";

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: card is a pointer-driven affordance; keyboard nav is handled by inner controls
    <article
      style={cardStyle}
      onClick={onCardClick}
      onContextMenu={onContextMenu}
      aria-label={project.name}
    >
      {/* Header row */}
      <div style={styles.headerRow}>
        {/* Apply visual distinction to project name when missing */}
        <span style={project.is_missing ? styles.projectNameMissing : styles.projectName}>
          {project.name}
        </span>
        <div style={styles.headerRight}>
          {activeRun && (
            <button
              type="button"
              style={styles.runningBadge}
              aria-label="Running"
              onClick={(e) => e.stopPropagation()}
            >
              Running
            </button>
          )}
          {/* Disable quick-run when project directory is missing */}
          <button
            type="button"
            title={quickRunTitle}
            aria-label={quickRunTitle}
            style={styles.quickRunBtn}
            onClick={(e) => {
              e.stopPropagation();
              onQuickRun?.();
            }}
            disabled={project.is_missing}
          >
            <Zap size={14} />
          </button>
        </div>
      </div>

      {/* Tags row — only if tags exist */}
      {project.tags.length > 0 && (
        <div style={styles.tagsRow}>
          {project.tags.map((tag) => (
            <span key={tag} style={styles.tagChip}>
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Missing state replaces git/meta rows */}
      {project.is_missing ? (
        <>
          <div style={styles.missingBanner} role="alert">
            <AlertTriangle size={14} style={styles.missingIcon} />
            Project directory not found
          </div>
          {/* Relocate / Remove action buttons (shown when callbacks provided) */}
          {(onRelocate || onRemove) && (
            <div style={styles.missingActions}>
              {onRelocate && (
                <button type="button" style={styles.missingActionBtn} onClick={onRelocate}>
                  Relocate...
                </button>
              )}
              {onRemove && (
                <button type="button" style={styles.missingActionBtn} onClick={onRemove}>
                  Remove
                </button>
              )}
            </div>
          )}
        </>
      ) : (
        <>
          {/* Git row */}
          <div style={styles.gitRow}>
            <div style={styles.gitBranch}>
              <GitBranch size={12} style={styles.gitBranchIcon} />
              <span style={styles.gitBranchName}>{gitStatus?.branch ?? "—"}</span>
            </div>
            <span style={styles.gitBadge}>{gitBadgeText(gitStatus)}</span>
          </div>

          {/* Meta row */}
          <div style={styles.metaRow}>
            <div style={styles.lastRunInfo}>
              {lastRun === undefined ? (
                <span style={styles.metaText}>Loading...</span>
              ) : lastRun === null ? (
                <span style={styles.metaText}>Never</span>
              ) : (
                <>
                  <span style={styles.metaText}>{formatRelative(lastRun.started_at)}</span>
                  <OutcomeBadge status={lastRun.status} />
                </>
              )}
            </div>
            <span style={styles.pathText} title={project.path}>
              {project.path}
            </span>
          </div>
        </>
      )}
    </article>
  );
});

export default ProjectCard;

// ---------------------------------------------------------------------------
// OutcomeBadge (internal helper) — uses module-level OUTCOME_STYLES lookup
// ---------------------------------------------------------------------------

function OutcomeBadge({ status }: { status: RunStatus }) {
  return (
    <span style={OUTCOME_STYLES[status]} aria-label={`Run status: ${status}`}>
      {status}
    </span>
  );
}

// ---------------------------------------------------------------------------
// ProjectCardSkeleton
// ---------------------------------------------------------------------------

export function ProjectCardSkeleton() {
  return (
    <article style={styles.skeletonCard} aria-busy="true" aria-label="Loading project">
      <div style={styles.skeletonHeader}>
        <div style={styles.skeletonName} />
        <div style={styles.skeletonBtn} />
      </div>
      <div style={styles.skeletonGit} />
      <div style={styles.skeletonMeta} />
    </article>
  );
}

// ---------------------------------------------------------------------------
// Inline styles
// ---------------------------------------------------------------------------

// Shared base for project name — avoids duplication between normal and missing variants
const projectNameBase: React.CSSProperties = {
  fontSize: "var(--font-size-base)",
  // Correct cast for fontWeight
  fontWeight: "var(--font-weight-semibold)" as React.CSSProperties["fontWeight"],
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap" as const,
  flex: 1,
};

const styles: Record<string, React.CSSProperties> = {
  card: {
    background: "var(--bg-surface)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-lg)",
    borderLeft: "4px solid var(--text-disabled)",
    padding: "var(--space-3) var(--space-4)",
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-2)",
    cursor: "pointer",
    transition: "background var(--duration-fast) var(--easing-out)",
    color: "var(--text-primary)",
    userSelect: "none",
  },
  headerRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--space-2)",
  },
  projectName: {
    ...projectNameBase,
    color: "var(--text-primary)",
  },
  // Visually distinguish missing project name
  projectNameMissing: {
    ...projectNameBase,
    color: "var(--text-disabled)",
    textDecoration: "line-through",
  },
  headerRight: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    flexShrink: 0,
  },
  runningBadge: {
    fontSize: "var(--font-size-xs)",
    color: "var(--running)",
    background: "transparent",
    border: "1px solid var(--running)",
    borderRadius: "var(--radius-sm)",
    padding: "1px 6px",
    cursor: "pointer",
    // Correct cast for fontWeight
    fontWeight: "var(--font-weight-semibold)" as React.CSSProperties["fontWeight"],
    ...(reducedMotion ? {} : { animation: "pc-pulse 1.5s ease-in-out infinite" }),
  },
  quickRunBtn: {
    background: "transparent",
    border: "none",
    padding: "2px",
    cursor: "pointer",
    color: "var(--accent)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: "var(--radius-sm)",
    transition: "color var(--duration-fast) var(--easing-out)",
    lineHeight: 1,
  },
  tagsRow: {
    display: "flex",
    flexWrap: "wrap" as const,
    gap: "var(--space-1)",
  },
  tagChip: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-secondary)",
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-sm)",
    padding: "1px 6px",
  },
  missingBanner: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    fontSize: "var(--font-size-sm)",
    color: "var(--error)",
    background: "var(--error-dim)",
    borderRadius: "var(--radius-sm)",
    padding: "var(--space-2) var(--space-3)",
  },
  missingIcon: {
    flexShrink: 0,
    color: "var(--error)",
  },
  // Action buttons row in missing state
  missingActions: {
    display: "flex",
    gap: "var(--space-2)",
  },
  missingActionBtn: {
    fontSize: "var(--font-size-xs)",
    background: "transparent",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-sm)",
    padding: "2px 8px",
    cursor: "pointer",
    color: "var(--text-secondary)",
  },
  gitRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--space-2)",
  },
  gitBranch: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-1)",
    overflow: "hidden",
  },
  gitBranchIcon: {
    color: "var(--text-secondary)",
    flexShrink: 0,
  },
  gitBranchName: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-secondary)",
    fontFamily: "var(--font-mono, monospace)",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap" as const,
  },
  gitBadge: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-secondary)",
    flexShrink: 0,
  },
  metaRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--space-2)",
  },
  lastRunInfo: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-1)",
    flexShrink: 0,
  },
  metaText: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-secondary)",
  },
  pathText: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-secondary)",
    fontFamily: "var(--font-mono, monospace)",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap" as const,
    direction: "rtl" as const,
    textAlign: "right" as const,
    flex: 1,
  },
  // Skeleton card
  skeletonCard: {
    background: "var(--bg-surface)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-lg)",
    borderLeft: "4px solid var(--text-disabled)",
    padding: "var(--space-3) var(--space-4)",
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-2)",
  },
  skeletonHeader: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
  },
  skeletonName: {
    height: "14px",
    width: "60%",
    background: "var(--bg-elevated)",
    borderRadius: "var(--radius-sm)",
    ...skeletonAnimation,
  },
  skeletonBtn: {
    height: "14px",
    width: "24px",
    background: "var(--bg-elevated)",
    borderRadius: "var(--radius-sm)",
    ...skeletonAnimation,
  },
  skeletonGit: {
    height: "12px",
    width: "40%",
    background: "var(--bg-elevated)",
    borderRadius: "var(--radius-sm)",
    ...skeletonAnimation,
  },
  skeletonMeta: {
    height: "12px",
    width: "80%",
    background: "var(--bg-elevated)",
    borderRadius: "var(--radius-sm)",
    ...skeletonAnimation,
  },
};
