// Unit tests for Dashboard route component CLI-check useEffect. See docs/tasks/T1.3.md.
import { render, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Mocks — declared before dynamic imports
// ---------------------------------------------------------------------------

vi.mock("../hooks/useProjects", () => ({
  useProjects: () => ({ projects: [], isLoading: false, error: null, refetch: vi.fn() }),
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

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(null),
}));

vi.mock("../ipc/commands", () => ({
  verifyClaudeCli: vi.fn(),
}));

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

// Import subjects AFTER mocks are registered.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { verifyClaudeCli } from "../ipc/commands";
import Dashboard from "./Dashboard";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderDashboard() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

// ---------------------------------------------------------------------------
// Reset between tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Test suites
// ---------------------------------------------------------------------------

describe("Dashboard — CLI check on mount", () => {
  it("calls verifyClaudeCli(undefined) on mount", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/bin/claude",
      version: "1.0.0",
      error: null,
    });

    renderDashboard();

    await waitFor(() => {
      expect(verifyClaudeCli).toHaveBeenCalledWith(undefined);
    });
  });

  it("does not navigate when CLI is found", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/bin/claude",
      version: "1.0.0",
      error: null,
    });

    renderDashboard();

    await waitFor(() => {
      expect(verifyClaudeCli).toHaveBeenCalledOnce();
    });

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it("navigates to /setup with replace:true when CLI is not found", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: false,
      resolved_path: null,
      version: null,
      error: "not found",
    });

    renderDashboard();

    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith("/setup", { replace: true });
    });
  });

  it("does not navigate when verifyClaudeCli rejects (IPC error)", async () => {
    vi.mocked(verifyClaudeCli).mockRejectedValue(new Error("IPC error"));

    renderDashboard();

    // Give the promise time to reject
    await waitFor(() => {
      expect(verifyClaudeCli).toHaveBeenCalledOnce();
    });

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it("calls verifyClaudeCli only once on mount (not on re-render)", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/bin/claude",
      version: "1.0.0",
      error: null,
    });

    const { rerender } = renderDashboard();
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    rerender(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <Dashboard />
        </MemoryRouter>
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(verifyClaudeCli).toHaveBeenCalledOnce();
    });
  });
});
