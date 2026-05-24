// Unit tests for ProjectCard component. See docs/tasks/T2.5.md.
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GitStatus, Project, Run } from "../ipc/bindings";
import { ProjectCard, ProjectCardSkeleton } from "./ProjectCard";

// ---------------------------------------------------------------------------
// Mock data factories
// ---------------------------------------------------------------------------

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: "proj-1",
    name: "my-app",
    path: "/home/user/projects/my-app",
    tags: [],
    language: "TypeScript",
    package_manager: "pnpm",
    added_at: "2026-01-01T00:00:00Z",
    last_modified: null,
    is_missing: false,
    ...overrides,
  };
}

function makeGitStatus(overrides: Partial<GitStatus> = {}): GitStatus {
  return {
    branch: "main",
    is_clean: true,
    dirty_files: 0,
    ahead: 0,
    behind: 0,
    last_polled: "2026-01-01T00:00:00Z",
    error: null,
    ...overrides,
  };
}

function makeRun(overrides: Partial<Run> = {}): Run {
  return {
    id: "run-1",
    project_id: "proj-1",
    project_path: "/home/user/projects/my-app",
    sequence_name: "build",
    attached_md_path: null,
    started_at: new Date(Date.now() - 5 * 60 * 1000).toISOString(), // 5m ago
    ended_at: null,
    status: "Completed",
    exit_code: 0,
    pid: null,
    note: null,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Reset mocks between tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Normal state
// ---------------------------------------------------------------------------

describe("ProjectCard — normal state", () => {
  it("renders project name", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={makeRun()} />);
    expect(screen.getByText("my-app")).toBeTruthy();
  });

  it("renders project path", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={makeRun()} />);
    expect(screen.getByTitle("/home/user/projects/my-app")).toBeTruthy();
  });

  it("renders git branch name", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ branch: "feature/xyz" })}
        lastRun={makeRun()}
      />
    );
    expect(screen.getByText("feature/xyz")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Git left-edge colors
// ---------------------------------------------------------------------------

describe("ProjectCard — git left-edge color", () => {
  it("clean git → left edge style has --success color", () => {
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({
          is_clean: true,
          dirty_files: 0,
          ahead: 0,
          behind: 0,
          error: null,
        })}
        lastRun={null}
      />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.style.borderLeftColor).toBe("var(--success)");
  });

  it("dirty git → left edge style has --error color", () => {
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ is_clean: false, dirty_files: 3 })}
        lastRun={null}
      />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.style.borderLeftColor).toBe("var(--error)");
  });

  it("ahead → left edge has --warning color", () => {
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ is_clean: false, dirty_files: 0, ahead: 2 })}
        lastRun={null}
      />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.style.borderLeftColor).toBe("var(--warning)");
  });

  it("git error → left edge has --error color", () => {
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ error: "fatal: not a git repo" })}
        lastRun={null}
      />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.style.borderLeftColor).toBe("var(--error)");
  });

  it("gitStatus undefined (loading) → left edge has --text-disabled color", () => {
    const { container } = render(
      <ProjectCard project={makeProject()} gitStatus={undefined} lastRun={null} />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.style.borderLeftColor).toBe("var(--text-disabled)");
  });
});

// ---------------------------------------------------------------------------
// Git badge text
// ---------------------------------------------------------------------------

describe("ProjectCard — git badge text", () => {
  it("gitStatus undefined → shows 'Loading...' badge", () => {
    render(<ProjectCard project={makeProject()} gitStatus={undefined} lastRun={null} />);
    expect(screen.getByText("Loading...")).toBeTruthy();
  });

  it("clean git → shows 'Clean'", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={null} />);
    expect(screen.getByText("Clean")).toBeTruthy();
  });

  it("dirty git → shows 'Dirty (N files)'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ is_clean: false, dirty_files: 5 })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Dirty (5 files)")).toBeTruthy();
  });

  it("git error → shows 'Error'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ error: "fatal" })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Error")).toBeTruthy();
  });

  it("ahead → shows 'Ahead N'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ ahead: 3, dirty_files: 0 })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Ahead 3")).toBeTruthy();
  });

  it("behind → shows 'Behind N'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ behind: 2, dirty_files: 0 })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Behind 2")).toBeTruthy();
  });

  it("ahead and behind → shows 'Ahead N · Behind N'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ ahead: 1, behind: 2, dirty_files: 0 })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Ahead 1 · Behind 2")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Missing state
// ---------------------------------------------------------------------------

describe("ProjectCard — missing state", () => {
  it("shows 'Project directory not found' banner", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.getByText("Project directory not found")).toBeTruthy();
  });

  it("hides git row when missing", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.queryByText("Clean")).toBeNull();
    expect(screen.queryByText("main")).toBeNull();
  });

  it("missing banner has role=alert", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.getByRole("alert")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Quick-run button tooltip
// ---------------------------------------------------------------------------

describe("ProjectCard — quick-run button", () => {
  it("title='Quick-run last sequence' when lastRun is a Run object", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={makeRun()} />);
    expect(screen.getByTitle("Quick-run last sequence")).toBeTruthy();
  });

  it("title='Pick a sequence' when lastRun === null", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={null} />);
    expect(screen.getByTitle("Pick a sequence")).toBeTruthy();
  });

  it("title='Quick-run last sequence' when lastRun is a Run (aria-label check)", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={makeRun()} />);
    expect(screen.getByRole("button", { name: "Quick-run last sequence" })).toBeTruthy();
  });

  it("title='Pick a sequence' when lastRun === null (aria-label check)", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={null} />);
    expect(screen.getByRole("button", { name: "Pick a sequence" })).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

describe("ProjectCard — tags", () => {
  it("renders tag chips when project.tags is non-empty", () => {
    render(
      <ProjectCard
        project={makeProject({ tags: ["react", "typescript"] })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.getByText("react")).toBeTruthy();
    expect(screen.getByText("typescript")).toBeTruthy();
  });

  it("does not render tag chips when project.tags is empty", () => {
    render(
      <ProjectCard project={makeProject({ tags: [] })} gitStatus={makeGitStatus()} lastRun={null} />
    );
    // FIX-9: test by asserting no tag text is present (inline styles never contain key names)
    expect(screen.queryByText("react")).toBeNull();
    expect(screen.queryByText("typescript")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Last run states
// ---------------------------------------------------------------------------

describe("ProjectCard — last run", () => {
  it("shows 'Never' text when lastRun=null", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={null} />);
    expect(screen.getByText("Never")).toBeTruthy();
  });

  it("shows 'Completed' badge when lastRun.status='Completed'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Completed" })}
      />
    );
    expect(screen.getByText("Completed")).toBeTruthy();
  });

  it("shows 'Failed' badge when lastRun.status='Failed'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Failed" })}
      />
    );
    expect(screen.getByText("Failed")).toBeTruthy();
  });

  it("shows 'Stopped' badge when lastRun.status='Stopped'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Stopped" })}
      />
    );
    expect(screen.getByText("Stopped")).toBeTruthy();
  });

  it("shows 'Pending' badge when lastRun.status='Pending'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Pending" })}
      />
    );
    expect(screen.getByText("Pending")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

describe("ProjectCard — event handlers", () => {
  it("calls onCardClick when card body is clicked", () => {
    const onCardClick = vi.fn();
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={null}
        onCardClick={onCardClick}
      />
    );
    fireEvent.click(container.firstChild as HTMLElement);
    expect(onCardClick).toHaveBeenCalledTimes(1);
  });

  it("calls onQuickRun when ⚡ button is clicked", () => {
    const onQuickRun = vi.fn();
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun()}
        onQuickRun={onQuickRun}
      />
    );
    fireEvent.click(screen.getByTitle("Quick-run last sequence"));
    expect(onQuickRun).toHaveBeenCalledTimes(1);
  });

  it("does NOT fire onCardClick when ⚡ button is clicked (stopPropagation)", () => {
    const onCardClick = vi.fn();
    const onQuickRun = vi.fn();
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun()}
        onCardClick={onCardClick}
        onQuickRun={onQuickRun}
      />
    );
    fireEvent.click(screen.getByTitle("Quick-run last sequence"));
    expect(onQuickRun).toHaveBeenCalledTimes(1);
    expect(onCardClick).not.toHaveBeenCalled();
  });

  it("calls onContextMenu on right-click", () => {
    const onContextMenu = vi.fn();
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={null}
        onContextMenu={onContextMenu}
      />
    );
    fireEvent.contextMenu(container.firstChild as HTMLElement);
    expect(onContextMenu).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// Running (activeRun) state
// ---------------------------------------------------------------------------

describe("ProjectCard — running state", () => {
  it("shows 'Running' badge when activeRun is set", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Running" })}
        activeRun={makeRun({ status: "Running" })}
      />
    );
    // The "Running" badge in the header
    expect(screen.getByRole("button", { name: "Running" })).toBeTruthy();
  });

  it("does not show 'Running' header badge when activeRun is null", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun()}
        activeRun={null}
      />
    );
    expect(screen.queryByRole("button", { name: "Running" })).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// ProjectCardSkeleton
// ---------------------------------------------------------------------------

describe("ProjectCardSkeleton", () => {
  it("renders without error", () => {
    const { container } = render(<ProjectCardSkeleton />);
    expect(container.firstChild).toBeTruthy();
  });

  it("has aria-busy=true", () => {
    render(<ProjectCardSkeleton />);
    expect(screen.getByRole("article", { name: "Loading project" })).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// NEW TESTS — gaps identified by tester pass
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Accessibility: card role and aria-label
// ---------------------------------------------------------------------------

describe("ProjectCard — accessibility", () => {
  it("card root has role=article", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={null} />);
    expect(screen.getByRole("article")).toBeTruthy();
  });

  it("card root aria-label is the project name", () => {
    render(
      <ProjectCard
        project={makeProject({ name: "coolproject" })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.getByRole("article", { name: "coolproject" })).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Git branch: null branch renders em-dash
// ---------------------------------------------------------------------------

describe("ProjectCard — git branch null", () => {
  it("renders '—' when gitStatus.branch is null", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ branch: null })}
        lastRun={null}
      />
    );
    expect(screen.getByText("—")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Git left-edge color: behind only → warning
// ---------------------------------------------------------------------------

describe("ProjectCard — git left-edge color (behind only)", () => {
  it("behind only → left edge has --warning color", () => {
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ dirty_files: 0, ahead: 0, behind: 3 })}
        lastRun={null}
      />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.style.borderLeftColor).toBe("var(--warning)");
  });
});

// ---------------------------------------------------------------------------
// Git badge: dirty_files count formatting
// ---------------------------------------------------------------------------

describe("ProjectCard — git badge dirty count", () => {
  it("dirty_files=1 → shows 'Dirty (1 files)'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ is_clean: false, dirty_files: 1 })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Dirty (1 files)")).toBeTruthy();
  });

  it("dirty_files=3 → shows 'Dirty (3 files)'", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus({ is_clean: false, dirty_files: 3 })}
        lastRun={null}
      />
    );
    expect(screen.getByText("Dirty (3 files)")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Meta row: lastRun === undefined → "Loading..." in meta
// ---------------------------------------------------------------------------

describe("ProjectCard — meta row loading state", () => {
  it("shows 'Loading...' in meta row when lastRun is undefined", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={undefined} />);
    // The git badge also shows "Clean" so we need to find specifically the meta Loading...
    // There can only be one "Loading..." when gitStatus is defined
    const allLoading = screen.getAllByText("Loading...");
    expect(allLoading.length).toBeGreaterThan(0);
  });

  it("shows 'Loading...' in git badge when gitStatus is undefined and lastRun is also undefined", () => {
    render(<ProjectCard project={makeProject()} gitStatus={undefined} lastRun={undefined} />);
    const allLoading = screen.getAllByText("Loading...");
    // Two "Loading..." elements: one for git badge, one for meta row
    expect(allLoading.length).toBe(2);
  });
});

// ---------------------------------------------------------------------------
// Running badge: stopPropagation — card click does not fire on Running badge click
// ---------------------------------------------------------------------------

describe("ProjectCard — running badge click stops propagation", () => {
  it("clicking 'Running' badge does not fire onCardClick", () => {
    const onCardClick = vi.fn();
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Running" })}
        activeRun={makeRun({ status: "Running" })}
        onCardClick={onCardClick}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "Running" }));
    expect(onCardClick).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Missing state: name still visible, tags still visible, quick-run still present
// ---------------------------------------------------------------------------

describe("ProjectCard — missing state completeness", () => {
  it("project name is still visible in missing state", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true, name: "lost-project" })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.getByText("lost-project")).toBeTruthy();
  });

  it("tag chips still visible in missing state when tags present", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true, tags: ["api", "rust"] })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.getByText("api")).toBeTruthy();
    expect(screen.getByText("rust")).toBeTruthy();
  });

  it("quick-run button is present but disabled with tooltip 'Project directory missing'", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    // FIX-6: disabled with tooltip overridden regardless of lastRun
    const btn = screen.getByTitle("Project directory missing");
    expect(btn).toBeTruthy();
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it("meta row is hidden in missing state (no 'Never' text)", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.queryByText("Never")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// onContextMenu: passes the event object through to the handler
// ---------------------------------------------------------------------------

describe("ProjectCard — onContextMenu event object", () => {
  it("onContextMenu receives the mouse event", () => {
    const onContextMenu = vi.fn();
    const { container } = render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={null}
        onContextMenu={onContextMenu}
      />
    );
    fireEvent.contextMenu(container.firstChild as HTMLElement);
    expect(onContextMenu).toHaveBeenCalledTimes(1);
    // Verify it received a MouseEvent-like object
    expect(onContextMenu.mock.calls[0][0]).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// Running state: Running badge is a button (clickable)
// ---------------------------------------------------------------------------

describe("ProjectCard — running badge is a button element", () => {
  it("Running badge has type=button", () => {
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ status: "Running" })}
        activeRun={makeRun({ status: "Running" })}
      />
    );
    const runningBtn = screen.getByRole("button", { name: "Running" });
    expect((runningBtn as HTMLButtonElement).type).toBe("button");
  });
});

// ---------------------------------------------------------------------------
// OutcomeBadge aria-label for all RunStatus values
// ---------------------------------------------------------------------------

describe("ProjectCard — OutcomeBadge aria-labels", () => {
  const statuses: Array<{
    status: "Completed" | "Failed" | "Stopped" | "Running" | "Pending";
    label: string;
  }> = [
    { status: "Completed", label: "Run status: Completed" },
    { status: "Failed", label: "Run status: Failed" },
    { status: "Stopped", label: "Run status: Stopped" },
    { status: "Running", label: "Run status: Running" },
    { status: "Pending", label: "Run status: Pending" },
  ];

  for (const { status, label } of statuses) {
    it(`aria-label="${label}" for status=${status}`, () => {
      render(
        <ProjectCard
          project={makeProject()}
          gitStatus={makeGitStatus()}
          lastRun={makeRun({ status })}
        />
      );
      expect(screen.getByLabelText(label)).toBeTruthy();
    });
  }
});

// ---------------------------------------------------------------------------
// ProjectCardSkeleton: explicit aria-busy attribute value
// ---------------------------------------------------------------------------

describe("ProjectCardSkeleton — aria attributes", () => {
  it("skeleton has aria-busy attribute set to 'true' (string)", () => {
    const { container } = render(<ProjectCardSkeleton />);
    const card = container.firstChild as HTMLElement;
    expect(card.getAttribute("aria-busy")).toBe("true");
  });

  it("skeleton has aria-label='Loading project'", () => {
    const { container } = render(<ProjectCardSkeleton />);
    const card = container.firstChild as HTMLElement;
    expect(card.getAttribute("aria-label")).toBe("Loading project");
  });
});

// ---------------------------------------------------------------------------
// formatRelative — timestamp display in meta row
// ---------------------------------------------------------------------------

describe("ProjectCard — formatRelative timestamp", () => {
  it("shows 'just now' for a run that started < 60s ago", () => {
    const startedAt = new Date(Date.now() - 10 * 1000).toISOString(); // 10s ago
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ started_at: startedAt })}
      />
    );
    expect(screen.getByText("just now")).toBeTruthy();
  });

  it("shows 'Xm ago' for a run that started 5 minutes ago", () => {
    const startedAt = new Date(Date.now() - 5 * 60 * 1000).toISOString();
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ started_at: startedAt })}
      />
    );
    expect(screen.getByText("5m ago")).toBeTruthy();
  });

  it("shows 'Xh ago' for a run that started 2 hours ago", () => {
    const startedAt = new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString();
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ started_at: startedAt })}
      />
    );
    expect(screen.getByText("2h ago")).toBeTruthy();
  });

  it("shows 'Xd ago' for a run that started 3 days ago", () => {
    const startedAt = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString();
    render(
      <ProjectCard
        project={makeProject()}
        gitStatus={makeGitStatus()}
        lastRun={makeRun({ started_at: startedAt })}
      />
    );
    expect(screen.getByText("3d ago")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Quick-run: lastRun undefined (still loading) — FIX-7: shows "Loading..."
// ---------------------------------------------------------------------------

describe("ProjectCard — quick-run title when lastRun undefined", () => {
  it("title='Loading...' when lastRun is undefined (loading state)", () => {
    render(<ProjectCard project={makeProject()} gitStatus={makeGitStatus()} lastRun={undefined} />);
    // FIX-7: undefined → "Loading..." (distinct from null → "Pick a sequence")
    expect(screen.getByTitle("Loading...")).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// FIX-6: missing state — name style, quick-run disabled, Relocate/Remove buttons
// ---------------------------------------------------------------------------

describe("ProjectCard — missing state FIX-6 enhancements", () => {
  it("project name has line-through style when is_missing", () => {
    const { container } = render(
      <ProjectCard
        project={makeProject({ is_missing: true, name: "gone-project" })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    // The name element should have textDecoration: line-through
    const nameEl = container.querySelector('[style*="line-through"]') as HTMLElement | null;
    expect(nameEl).not.toBeNull();
    expect(nameEl?.textContent).toBe("gone-project");
  });

  it("quick-run button aria-label is 'Project directory missing' when is_missing", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={makeRun()}
      />
    );
    // FIX-6: overrides lastRun-derived title
    expect(screen.getByTitle("Project directory missing")).toBeTruthy();
  });

  it("Relocate button rendered when onRelocate provided in missing state", () => {
    const onRelocate = vi.fn();
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
        onRelocate={onRelocate}
      />
    );
    const relocateBtn = screen.getByText("Relocate...");
    expect(relocateBtn).toBeTruthy();
    fireEvent.click(relocateBtn);
    expect(onRelocate).toHaveBeenCalledTimes(1);
  });

  it("Remove button rendered when onRemove provided in missing state", () => {
    const onRemove = vi.fn();
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
        onRemove={onRemove}
      />
    );
    const removeBtn = screen.getByText("Remove");
    expect(removeBtn).toBeTruthy();
    fireEvent.click(removeBtn);
    expect(onRemove).toHaveBeenCalledTimes(1);
  });

  it("no Relocate or Remove buttons rendered when callbacks not provided", () => {
    render(
      <ProjectCard
        project={makeProject({ is_missing: true })}
        gitStatus={makeGitStatus()}
        lastRun={null}
      />
    );
    expect(screen.queryByText("Relocate...")).toBeNull();
    expect(screen.queryByText("Remove")).toBeNull();
  });

  it("Relocate/Remove buttons not rendered when project is NOT missing", () => {
    const onRelocate = vi.fn();
    const onRemove = vi.fn();
    render(
      <ProjectCard
        project={makeProject({ is_missing: false })}
        gitStatus={makeGitStatus()}
        lastRun={null}
        onRelocate={onRelocate}
        onRemove={onRemove}
      />
    );
    expect(screen.queryByText("Relocate...")).toBeNull();
    expect(screen.queryByText("Remove")).toBeNull();
  });
});
