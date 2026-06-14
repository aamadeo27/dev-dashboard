import { useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Grid, List, Plus, Settings as SettingsIcon } from "lucide-react";
import type React from "react";
import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import ContextMenu, { type ContextMenuItem } from "../components/ContextMenu";
import LaunchModal from "../components/LaunchModal";
import { ProjectCard, ProjectCardSkeleton } from "../components/ProjectCard";
import RateLimitPill from "../components/RateLimitPill";
import TagEditorPopover from "../components/TagEditorPopover";
import { useGitStatus, useGitStatusListener, useVisibleProjects } from "../hooks/useGitStatus";
import { PROJECTS_QUERY_KEY, useProjects } from "../hooks/useProjects";
import { RUN_HISTORY_QUERY_KEY, useRunHistory } from "../hooks/useRunHistory";
import { useSettings } from "../hooks/useSettings";
import type { Run } from "../ipc/bindings";
import type { Project } from "../ipc/bindings";
import {
  addProject,
  openInEditor,
  openInTerminal,
  relocateProject,
  removeProject,
  verifyClaudeCli,
} from "../ipc/commands";

// ---------------------------------------------------------------------------
// Per-card wrapper — each card needs its own useGitStatus call
// ---------------------------------------------------------------------------

const ProjectCardWrapper = memo(function ProjectCardWrapper({
  project,
  confirmingRemove,
  onCardClick,
  onQuickRun,
  onContextMenu,
  onRelocate,
  onRemove,
  onConfirmRemove,
  onCancelRemove,
}: {
  project: Project;
  confirmingRemove: boolean;
  onCardClick: (id: string) => void;
  onQuickRun: (id: string) => void;
  onContextMenu: (e: React.MouseEvent, project: Project) => void;
  onRelocate: (id: string) => void;
  onRemove: (id: string) => void;
  onConfirmRemove: (id: string) => void;
  onCancelRemove: () => void;
}) {
  const gitStatus = useGitStatus(project.id);
  const { data: runs, isLoading: runsLoading } = useRunHistory(project.id);
  const lastRun: Run | null | undefined = runsLoading ? undefined : (runs?.[0] ?? null);
  const id = project.id;

  const handleCardClick = useCallback(() => onCardClick(id), [onCardClick, id]);
  const handleQuickRun = useCallback(() => onQuickRun(id), [onQuickRun, id]);
  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => onContextMenu(e, project),
    [onContextMenu, project]
  );
  const handleRelocate = useCallback(() => onRelocate(id), [onRelocate, id]);
  const handleRemove = useCallback(() => onRemove(id), [onRemove, id]);
  const handleConfirmRemove = useCallback(() => onConfirmRemove(id), [onConfirmRemove, id]);

  if (confirmingRemove) {
    return (
      <article
        aria-label={project.name}
        style={{
          background: "var(--bg-surface)",
          border: "1px solid var(--error)",
          borderRadius: "var(--radius-lg)",
          padding: "var(--space-4)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-3)",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "120px",
        }}
      >
        <p
          style={{
            color: "var(--text-primary)",
            fontSize: "var(--font-size-sm)",
            margin: 0,
            textAlign: "center",
          }}
        >
          Remove <strong>{project.name}</strong>?
        </p>
        <div style={{ display: "flex", gap: "var(--space-2)" }}>
          <button
            type="button"
            onClick={handleConfirmRemove}
            style={{
              background: "var(--error)",
              border: "none",
              borderRadius: "var(--radius-md)",
              padding: "var(--space-1) var(--space-3)",
              color: "#fff",
              fontSize: "var(--font-size-sm)",
              cursor: "pointer",
            }}
          >
            Remove
          </button>
          <button
            type="button"
            onClick={onCancelRemove}
            style={{
              background: "var(--bg-elevated)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-md)",
              padding: "var(--space-1) var(--space-3)",
              color: "var(--text-primary)",
              fontSize: "var(--font-size-sm)",
              cursor: "pointer",
            }}
          >
            Cancel
          </button>
        </div>
      </article>
    );
  }

  return (
    <ProjectCard
      project={project}
      gitStatus={gitStatus ?? undefined}
      lastRun={lastRun}
      activeRun={undefined}
      onCardClick={handleCardClick}
      onQuickRun={handleQuickRun}
      onContextMenu={handleContextMenu}
      onRelocate={project.is_missing ? handleRelocate : undefined}
      onRemove={project.is_missing ? handleRemove : undefined}
    />
  );
});

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

export default function Dashboard() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { projects, isLoading } = useProjects();
  const { settings, updateSettings } = useSettings();
  const [search, setSearch] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [searchFocused, setSearchFocused] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    project: Project;
    x: number;
    y: number;
  } | null>(null);
  const [tagEditorProject, setTagEditorProject] = useState<{
    project: Project;
    x: number;
    y: number;
  } | null>(null);
  const [confirmRemoveId, setConfirmRemoveId] = useState<string | null>(null);
  const [launchModal, setLaunchModal] = useState<{
    projectId: string;
    sequenceName: string;
  } | null>(null);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);
  const closeTagEditor = useCallback(() => setTagEditorProject(null), []);
  const handleCancelRemove = useCallback(() => setConfirmRemoveId(null), []);

  const handleRelocate = useCallback(
    async (projectId: string) => {
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return;
      const newPath = Array.isArray(selected) ? selected[0] : selected;
      try {
        await relocateProject(projectId, newPath);
        await queryClient.invalidateQueries({ queryKey: PROJECTS_QUERY_KEY });
      } catch {
        // backend emits toast:show on error
      }
    },
    [queryClient]
  );

  const handleConfirmRemove = useCallback(
    async (projectId: string) => {
      try {
        await removeProject(projectId);
        await queryClient.invalidateQueries({ queryKey: PROJECTS_QUERY_KEY });
      } catch {
        // backend emits toast:show on error
      }
      setConfirmRemoveId(null);
    },
    [queryClient]
  );

  const handleRemoveById = useCallback((projectId: string) => {
    setConfirmRemoveId(projectId);
  }, []);

  const handleContextMenu = useCallback((e: React.MouseEvent, project: Project) => {
    e.preventDefault();
    setContextMenu({ project, x: e.clientX, y: e.clientY });
  }, []);

  const handleCardClick = useCallback(
    (projectId: string) => navigate(`/projects/${projectId}`),
    [navigate]
  );

  const handleQuickRun = useCallback(
    (projectId: string) => {
      const runs = queryClient.getQueryData<Run[]>(RUN_HISTORY_QUERY_KEY(projectId));
      if (runs === undefined) return; // run data still loading → no-op
      if (runs.length === 0) {
        navigate(`/projects/${projectId}`, { state: { focusSequences: true } });
      } else {
        setLaunchModal({ projectId, sequenceName: runs[0].sequence_name });
      }
    },
    [queryClient, navigate]
  );

  const handleTagsChange = useCallback(
    (projectId: string, tags: string[]) => {
      queryClient.setQueryData<Project[]>(
        PROJECTS_QUERY_KEY,
        (old) => old?.map((p) => (p.id === projectId ? { ...p, tags } : p)) ?? []
      );
    },
    [queryClient]
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: navigate from useNavigate is stable; run only on mount
  useEffect(() => {
    verifyClaudeCli(undefined)
      .then((result) => {
        if (!result.found) navigate("/setup", { replace: true });
      })
      .catch(() => {});
  }, []);

  // Wire git:updated event bus once for the Dashboard lifetime:
  useGitStatusListener();

  const viewMode = settings?.view_mode ?? "Grid";

  // Union of all tags across all projects, sorted alphabetically:
  const allTags = useMemo(() => {
    const tagSet = new Set<string>();
    for (const p of projects) for (const t of p.tags) tagSet.add(t);
    return Array.from(tagSet).sort();
  }, [projects]);

  // Filter projects by search (name or path) and selected tag chips:
  const filteredProjects = useMemo(() => {
    const q = search.toLowerCase();
    return projects.filter((p) => {
      const matchesSearch =
        !q || p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q);
      const matchesTags =
        selectedTags.length === 0 || selectedTags.every((t) => p.tags.includes(t));
      return matchesSearch && matchesTags;
    });
  }, [projects, search, selectedTags]);

  // Report visible project ids to git poller (debounced by the hook):
  const visibleProjectIds = useMemo(() => filteredProjects.map((p) => p.id), [filteredProjects]);
  useVisibleProjects(visibleProjectIds);

  const menuItems = useMemo<ContextMenuItem[]>(() => {
    if (!contextMenu) return [];
    const p = contextMenu.project;
    return [
      { label: "Open in Editor", onClick: () => openInEditor(p.id).catch(() => {}) },
      { label: "Open in Terminal", onClick: () => openInTerminal(p.id).catch(() => {}) },
      {
        label: "Edit Tags",
        onClick: () => setTagEditorProject({ project: p, x: contextMenu.x, y: contextMenu.y }),
      },
      ...(p.is_missing ? [{ label: "Relocate", onClick: () => handleRelocate(p.id) }] : []),
      { label: "Remove", danger: true, onClick: () => setConfirmRemoveId(p.id) },
    ];
  }, [contextMenu, handleRelocate]);

  const handleAddProject = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected === null) return;
    const path = Array.isArray(selected) ? selected[0] : selected;
    try {
      await addProject(path);
      await queryClient.invalidateQueries({ queryKey: PROJECTS_QUERY_KEY });
    } catch {
      // Backend emits toast:show on error; no UI handling needed here.
    }
  };

  const handleToggleTag = (tag: string) => {
    setSelectedTags((prev) =>
      prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag]
    );
  };

  const handleToggleView = (mode: "Grid" | "List") => {
    updateSettings({
      view_mode: mode,
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: null,
      usage_poll_interval_secs: null,
      retention_days: null,
      retention_size_mb: null,
    });
  };

  // ---------------------------------------------------------------------------
  // Render helpers
  // ---------------------------------------------------------------------------

  function renderContent() {
    if (isLoading) {
      return (
        <div style={viewMode === "Grid" ? styles.gridLayout : styles.listLayout}>
          {Array.from({ length: 6 }).map((_, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: skeleton placeholders have no identity
            <ProjectCardSkeleton key={i} />
          ))}
        </div>
      );
    }

    if (projects.length === 0) {
      return (
        <div style={styles.emptyState}>
          <p style={styles.emptyText}>No projects yet.</p>
          <button type="button" style={styles.emptyAddBtn} onClick={handleAddProject}>
            Add your first project
          </button>
        </div>
      );
    }

    if (filteredProjects.length === 0) {
      return (
        <div style={styles.emptyState}>
          <p style={styles.emptyText}>No projects match your search.</p>
        </div>
      );
    }

    return (
      <div style={viewMode === "Grid" ? styles.gridLayout : styles.listLayout}>
        {filteredProjects.map((project) => (
          <ProjectCardWrapper
            key={project.id}
            project={project}
            confirmingRemove={confirmRemoveId === project.id}
            onCardClick={handleCardClick}
            onQuickRun={handleQuickRun}
            onContextMenu={handleContextMenu}
            onRelocate={handleRelocate}
            onRemove={handleRemoveById}
            onConfirmRemove={handleConfirmRemove}
            onCancelRemove={handleCancelRemove}
          />
        ))}
      </div>
    );
  }

  return (
    <div style={styles.root}>
      {/* Top bar */}
      <div style={styles.topbar} className="db-topbar">
        <span style={styles.appName}>dev-dashboard</span>
        <RateLimitPill />
        <button
          type="button"
          style={styles.settingsBtn}
          onClick={() => navigate("/settings")}
          aria-label="Settings"
        >
          <SettingsIcon size={18} />
        </button>
      </div>

      {/* Toolbar */}
      <div style={styles.toolbar} className="db-toolbar">
        {/* Add Project */}
        <button
          type="button"
          style={styles.addProjectBtn}
          onClick={handleAddProject}
          onMouseEnter={(e) => {
            (e.currentTarget as HTMLButtonElement).style.background = "var(--bg-hover)";
          }}
          onMouseLeave={(e) => {
            (e.currentTarget as HTMLButtonElement).style.background = "var(--bg-elevated)";
          }}
        >
          <Plus size={14} />
          {" Add Project"}
        </button>

        {/* Search */}
        <input
          type="text"
          placeholder="Search projects..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          onFocus={() => setSearchFocused(true)}
          onBlur={() => setSearchFocused(false)}
          style={{
            ...styles.searchInput,
            borderColor: searchFocused ? "var(--border-strong)" : "var(--border-subtle)",
          }}
          aria-label="Search projects"
        />

        {/* Tag chips */}
        {allTags.map((tag) => {
          const active = selectedTags.includes(tag);
          return (
            <button
              key={tag}
              type="button"
              onClick={() => handleToggleTag(tag)}
              style={active ? styles.tagChipActive : styles.tagChipInactive}
            >
              {tag}
            </button>
          );
        })}

        {/* View toggle */}
        <div style={styles.viewToggle}>
          <button
            type="button"
            style={{
              ...styles.viewBtn,
              color: viewMode === "Grid" ? "var(--primary)" : "var(--text-secondary)",
            }}
            onClick={() => handleToggleView("Grid")}
            aria-label="Grid view"
            aria-pressed={viewMode === "Grid"}
          >
            <Grid size={16} />
          </button>
          <div style={styles.viewDivider} />
          <button
            type="button"
            style={{
              ...styles.viewBtn,
              color: viewMode === "List" ? "var(--primary)" : "var(--text-secondary)",
            }}
            onClick={() => handleToggleView("List")}
            aria-label="List view"
            aria-pressed={viewMode === "List"}
          >
            <List size={16} />
          </button>
        </div>
      </div>

      {/* Content */}
      <div style={styles.content} className="db-content">
        {renderContent()}
      </div>

      {/* Overlays */}
      {contextMenu && (
        <ContextMenu
          items={menuItems}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={closeContextMenu}
        />
      )}
      {tagEditorProject && (
        <TagEditorPopover
          project={tagEditorProject.project}
          x={tagEditorProject.x}
          y={tagEditorProject.y}
          onClose={closeTagEditor}
          onTagsChange={handleTagsChange}
        />
      )}
      {launchModal && (
        <LaunchModal
          projectId={launchModal.projectId}
          sequenceName={launchModal.sequenceName}
          onClose={() => setLaunchModal(null)}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Inline styles
// ---------------------------------------------------------------------------

const styles: Record<string, React.CSSProperties> = {
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    background: "var(--bg-base)",
    color: "var(--text-primary)",
    overflow: "hidden",
  },
  topbar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    background: "var(--bg-surface)",
    paddingTop: "var(--space-4)",
    paddingBottom: "var(--space-4)",
    paddingLeft: "var(--space-6)",
    paddingRight: "var(--space-6)",
    borderBottom: "1px solid var(--border-subtle)",
  },
  appName: {
    fontSize: "var(--font-size-lg)",
    fontWeight: "var(--font-weight-bold)" as React.CSSProperties["fontWeight"],
    color: "var(--primary)",
  },
  settingsBtn: {
    background: "transparent",
    border: "none",
    cursor: "pointer",
    color: "var(--text-secondary)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    padding: "var(--space-1)",
    borderRadius: "var(--radius-md)",
  },
  toolbar: {
    display: "flex",
    flexWrap: "wrap" as const,
    alignItems: "center",
    gap: "var(--space-2)",
    paddingTop: "var(--space-3)",
    paddingBottom: "var(--space-3)",
    paddingLeft: "var(--space-6)",
    paddingRight: "var(--space-6)",
    borderBottom: "1px solid var(--border-subtle)",
    background: "var(--bg-base)",
  },
  addProjectBtn: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-1)",
    background: "var(--bg-elevated)",
    border: "1px solid var(--primary)",
    color: "var(--text-primary)",
    borderRadius: "var(--radius-md)",
    paddingTop: "var(--space-2)",
    paddingBottom: "var(--space-2)",
    paddingLeft: "var(--space-3)",
    paddingRight: "var(--space-3)",
    cursor: "pointer",
    fontSize: "var(--font-size-sm)",
    transition: "background var(--duration-fast) var(--easing-out)",
  },
  searchInput: {
    flex: 1,
    minWidth: "140px",
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    color: "var(--text-primary)",
    borderRadius: "var(--radius-md)",
    paddingTop: "var(--space-2)",
    paddingBottom: "var(--space-2)",
    paddingLeft: "var(--space-3)",
    paddingRight: "var(--space-3)",
    fontSize: "var(--font-size-sm)",
    outline: "none",
    transition: "border-color var(--duration-fast) var(--easing-out)",
  },
  tagChipActive: {
    borderRadius: "var(--radius-sm)",
    paddingTop: "var(--space-1)",
    paddingBottom: "var(--space-1)",
    paddingLeft: "var(--space-2)",
    paddingRight: "var(--space-2)",
    fontSize: "var(--font-size-xs)",
    cursor: "pointer",
    background: "var(--primary-dim)",
    border: "1px solid var(--primary)",
    color: "var(--primary)",
  },
  tagChipInactive: {
    borderRadius: "var(--radius-sm)",
    paddingTop: "var(--space-1)",
    paddingBottom: "var(--space-1)",
    paddingLeft: "var(--space-2)",
    paddingRight: "var(--space-2)",
    fontSize: "var(--font-size-xs)",
    cursor: "pointer",
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    color: "var(--text-secondary)",
  },
  viewToggle: {
    display: "flex",
    alignItems: "center",
    marginLeft: "auto",
  },
  viewBtn: {
    background: "transparent",
    border: "none",
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    padding: "var(--space-1)",
    borderRadius: "var(--radius-sm)",
    transition: "color var(--duration-fast) var(--easing-out)",
  },
  viewDivider: {
    width: "1px",
    height: "16px",
    background: "var(--border-subtle)",
    margin: "0 var(--space-1)",
  },
  content: {
    flex: 1,
    overflowY: "auto" as const,
    padding: "var(--space-6)",
  },
  gridLayout: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
    gap: "var(--space-4)",
  },
  listLayout: {
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-3)",
  },
  emptyState: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: "var(--space-3)",
    paddingTop: "var(--space-12)",
  },
  emptyText: {
    color: "var(--text-secondary)",
    margin: 0,
    fontSize: "var(--font-size-base)",
  },
  emptyAddBtn: {
    background: "var(--bg-elevated)",
    border: "1px solid var(--primary)",
    color: "var(--primary)",
    borderRadius: "var(--radius-md)",
    paddingTop: "var(--space-2)",
    paddingBottom: "var(--space-2)",
    paddingLeft: "var(--space-4)",
    paddingRight: "var(--space-4)",
    cursor: "pointer",
    fontSize: "var(--font-size-sm)",
  },
};
