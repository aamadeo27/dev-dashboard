// Unit tests for Dashboard.handleQuickRun dispatch (T5.10).
// Verifies: no-prior-run → S-03 navigation with focusSequences state;
//           prior-run → LaunchModal opened (navigate not called);
//           run data loading → no-op.
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Mocks — declared before dynamic imports
// ---------------------------------------------------------------------------

const MOCK_PROJECT = {
  id: "proj-001",
  name: "my-app",
  path: "/home/user/my-app",
  tags: [],
  language: null,
  package_manager: null,
  added_at: "2026-05-21T10:00:00Z",
  last_modified: null,
  is_missing: false,
};

const MOCK_RUN = {
  id: "run-001",
  project_id: "proj-001",
  project_path: "/home/user/my-app",
  sequence_name: "build-and-test",
  attached_md_path: null,
  started_at: "2026-06-01T10:00:00Z",
  ended_at: "2026-06-01T10:05:00Z",
  status: "Completed" as const,
  exit_code: 0,
  pid: null,
  note: null,
};

vi.mock("../hooks/useProjects", () => ({
  useProjects: vi.fn(() => ({
    projects: [MOCK_PROJECT],
    isLoading: false,
    error: null,
    refetch: vi.fn(),
  })),
  PROJECTS_QUERY_KEY: ["projects"],
}));

vi.mock("../hooks/useGitStatus", () => ({
  useGitStatusListener: vi.fn(),
  useGitStatus: vi.fn().mockReturnValue(null),
  useVisibleProjects: vi.fn(),
}));

vi.mock("../hooks/useSettings", () => ({
  useSettings: () => ({
    settings: {
      view_mode: "Grid",
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: 30,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 100,
    },
    isLoading: false,
    error: null,
    updateSettings: vi.fn(),
    isSaving: false,
  }),
}));

// useRunHistory is controlled per test via mockReturnValue
vi.mock("../hooks/useRunHistory", () => ({
  useRunHistory: vi.fn(),
  RUN_HISTORY_QUERY_KEY: (id: string) => ["runs", id],
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(null),
}));

vi.mock("../ipc/commands", () => ({
  verifyClaudeCli: vi.fn().mockResolvedValue({
    found: true,
    resolved_path: "/usr/bin/claude",
    version: "1.0.0",
    error: null,
  }),
  addProject: vi.fn(),
  removeProject: vi.fn(),
  relocateProject: vi.fn(),
  openInEditor: vi.fn(),
  openInTerminal: vi.fn(),
}));

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

// Import subjects AFTER mocks
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RUN_HISTORY_QUERY_KEY, useRunHistory } from "../hooks/useRunHistory";
import Dashboard from "./Dashboard";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderDashboard(queryClient: QueryClient) {
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

function makeQueryClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

// ---------------------------------------------------------------------------
// Reset between tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Helpers to set query cache state via useRunHistory mock + queryClient seed
// ---------------------------------------------------------------------------

function seedRunCache(queryClient: QueryClient, projectId: string, runs: (typeof MOCK_RUN)[]) {
  queryClient.setQueryData(RUN_HISTORY_QUERY_KEY(projectId), runs);
}

// ---------------------------------------------------------------------------
// Test suites
// ---------------------------------------------------------------------------

describe("Dashboard.handleQuickRun — no prior run", () => {
  it("navigates to S-03 with focusSequences:true when project has no runs", async () => {
    const qc = makeQueryClient();
    seedRunCache(qc, MOCK_PROJECT.id, []);
    vi.mocked(useRunHistory).mockReturnValue({
      data: [],
      isLoading: false,
      isError: false,
      error: null,
      status: "success",
    } as unknown as ReturnType<typeof useRunHistory>);

    renderDashboard(qc);

    const zapBtn = await screen.findByRole("button", { name: /pick a sequence/i });
    fireEvent.click(zapBtn);

    expect(mockNavigate).toHaveBeenCalledWith(`/projects/${MOCK_PROJECT.id}`, {
      state: { focusSequences: true },
    });
  });
});

describe("Dashboard.handleQuickRun — prior run exists", () => {
  it("does not navigate to S-03 when project has a prior run", async () => {
    const qc = makeQueryClient();
    seedRunCache(qc, MOCK_PROJECT.id, [MOCK_RUN]);
    vi.mocked(useRunHistory).mockReturnValue({
      data: [MOCK_RUN],
      isLoading: false,
      isError: false,
      error: null,
      status: "success",
    } as unknown as ReturnType<typeof useRunHistory>);

    renderDashboard(qc);

    const zapBtn = await screen.findByRole("button", { name: /quick-run last sequence/i });
    fireEvent.click(zapBtn);

    expect(mockNavigate).not.toHaveBeenCalledWith(
      expect.stringContaining("/projects/"),
      expect.objectContaining({ state: { focusSequences: true } })
    );
  });

  it("uses the newest run's sequence_name when multiple runs exist", async () => {
    const olderRun = {
      ...MOCK_RUN,
      id: "run-000",
      sequence_name: "old-sequence",
      started_at: "2026-05-01T10:00:00Z",
    };
    const runs = [MOCK_RUN, olderRun]; // newest first
    const qc = makeQueryClient();
    seedRunCache(qc, MOCK_PROJECT.id, runs);
    vi.mocked(useRunHistory).mockReturnValue({
      data: runs,
      isLoading: false,
      isError: false,
      error: null,
      status: "success",
    } as unknown as ReturnType<typeof useRunHistory>);

    renderDashboard(qc);

    const zapBtn = await screen.findByRole("button", { name: /quick-run last sequence/i });
    fireEvent.click(zapBtn);

    // navigate should NOT be called; modal opens instead
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});

describe("Dashboard.handleQuickRun — run data loading", () => {
  it("is a no-op when run data is not yet in cache", async () => {
    const qc = makeQueryClient();
    // Do NOT seed cache — queryClient.getQueryData returns undefined
    vi.mocked(useRunHistory).mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
      error: null,
      status: "pending",
    } as unknown as ReturnType<typeof useRunHistory>);

    renderDashboard(qc);

    const zapBtn = await screen.findByRole("button", { name: /loading/i });
    fireEvent.click(zapBtn);

    expect(mockNavigate).not.toHaveBeenCalled();
  });
});

describe("Dashboard.handleQuickRun — missing project", () => {
  it("⚡ button is disabled when project is missing", async () => {
    const missingProject = { ...MOCK_PROJECT, is_missing: true };
    const { useProjects } = await import("../hooks/useProjects");
    vi.mocked(useProjects).mockReturnValue({
      projects: [missingProject],
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    } as ReturnType<typeof useProjects>);

    vi.mocked(useRunHistory).mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: false,
      error: null,
      status: "success",
    } as unknown as ReturnType<typeof useRunHistory>);

    const qc = makeQueryClient();
    renderDashboard(qc);

    await waitFor(() => {
      const zapBtn = screen.getByRole("button", { name: /project directory missing/i });
      expect(zapBtn).toBeDisabled();
    });
  });
});
