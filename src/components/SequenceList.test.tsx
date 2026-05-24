// Unit tests for SequenceList component. See docs/tasks/T3.2.md.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Sequence } from "../ipc/bindings";

// ---------------------------------------------------------------------------
// Mocks — must be declared before dynamic imports
// ---------------------------------------------------------------------------

vi.mock("../ipc/commands", () => ({
  listSequences: vi.fn(),
}));

// Import after mock registration
import { listSequences } from "../ipc/commands";
import { SequenceList } from "./SequenceList";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeSequence(overrides: Partial<Sequence> = {}): Sequence {
  return {
    name: "build-and-test",
    description: "Runs build, lint, and unit tests.",
    path: "/home/user/.config/dev-dashboard/sequences/build-and-test.md",
    mtime: "2026-05-01T10:00:00Z",
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// QueryClient wrapper factory — fresh client per test to avoid cache leakage
// ---------------------------------------------------------------------------

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });
  return {
    queryClient,
    wrapper: ({ children }: { children: React.ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children),
  };
}

// ---------------------------------------------------------------------------
// Reset mocks
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("SequenceList — loading state", () => {
  it("1. shows 'Loading sequences...' while fetching", () => {
    // Never resolves — keeps hook in loading state
    vi.mocked(listSequences).mockImplementation(() => new Promise(() => {}));
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });
    expect(screen.getByText("Loading sequences...")).toBeTruthy();
  });
});

describe("SequenceList — error state", () => {
  it("2. shows error message when listSequences rejects", async () => {
    vi.mocked(listSequences).mockRejectedValue(new Error("NOT_FOUND: project not found"));
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-bad" }), { wrapper });
    await waitFor(() => {
      expect(screen.getByText("NOT_FOUND: project not found")).toBeTruthy();
    });
  });
});

describe("SequenceList — empty state", () => {
  it("3. shows empty state with directory hint for empty list", async () => {
    vi.mocked(listSequences).mockResolvedValue([]);
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });
    await waitFor(() => {
      expect(screen.getByText("No sequences found. Add .md files to .claude/sequences/ in your project.")).toBeTruthy();
    });
  });
});

describe("SequenceList — populated state", () => {
  it("4. renders correct number of SequenceRow items", async () => {
    const sequences = [
      makeSequence({ name: "seq-a", description: "First sequence." }),
      makeSequence({ name: "seq-b", description: "Second sequence." }),
      makeSequence({ name: "seq-c", description: "Third sequence." }),
    ];
    vi.mocked(listSequences).mockResolvedValue(sequences);
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });
    await waitFor(() => {
      expect(screen.getAllByRole("listitem").length).toBe(3);
    });
  });

  it("5. passes correct sequence name and description to rows", async () => {
    const sequences = [
      makeSequence({ name: "deploy", description: "Deploys to production." }),
      makeSequence({ name: "test-suite", description: "Runs all tests." }),
    ];
    vi.mocked(listSequences).mockResolvedValue(sequences);
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });
    await waitFor(() => {
      expect(screen.getByText("deploy")).toBeTruthy();
      expect(screen.getByText("Deploys to production.")).toBeTruthy();
      expect(screen.getByText("test-suite")).toBeTruthy();
      expect(screen.getByText("Runs all tests.")).toBeTruthy();
    });
  });

  it("6. clicking a row selects it (selected=true on that row, others false)", async () => {
    const sequences = [
      makeSequence({ name: "seq-a", description: "First." }),
      makeSequence({ name: "seq-b", description: "Second." }),
    ];
    vi.mocked(listSequences).mockResolvedValue(sequences);
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });

    await waitFor(() => {
      expect(screen.getAllByRole("listitem").length).toBe(2);
    });

    const rows = screen.getAllByRole("listitem");
    // Initially none are selected
    expect(rows[0].getAttribute("data-selected")).toBe("false");
    expect(rows[1].getAttribute("data-selected")).toBe("false");

    // Click the first row
    fireEvent.click(rows[0]);
    expect(rows[0].getAttribute("data-selected")).toBe("true");
    expect(rows[1].getAttribute("data-selected")).toBe("false");

    // Click the second row — first deselects
    fireEvent.click(rows[1]);
    expect(rows[0].getAttribute("data-selected")).toBe("false");
    expect(rows[1].getAttribute("data-selected")).toBe("true");
  });

  it("7. onRun stub fires console.log with correct sequence name", async () => {
    const consoleSpy = vi.spyOn(console, "log").mockImplementation(() => {});
    const seq = makeSequence({ name: "run-me", description: "Runs something." });
    vi.mocked(listSequences).mockResolvedValue([seq]);
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Run run-me" })).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: "Run run-me" }));
    expect(consoleSpy).toHaveBeenCalledWith("Run sequence:", "run-me");

    consoleSpy.mockRestore();
  });
});

describe("SequenceList — query key", () => {
  it("8. different projectId re-queries (queryKey changes)", async () => {
    const seqsA = [makeSequence({ name: "seq-for-proj-a", description: "For A." })];
    const seqsB = [
      makeSequence({ name: "seq-for-proj-b-1", description: "For B first." }),
      makeSequence({ name: "seq-for-proj-b-2", description: "For B second." }),
    ];

    vi.mocked(listSequences)
      .mockResolvedValueOnce(seqsA)
      .mockResolvedValueOnce(seqsB);

    const { wrapper, queryClient } = makeWrapper();

    // Render for project A
    const { rerender } = render(createElement(SequenceList, { projectId: "proj-a" }), { wrapper });

    await waitFor(() => {
      expect(screen.getByText("seq-for-proj-a")).toBeTruthy();
    });

    // Invalidate to force re-fetch on new project
    queryClient.clear();

    // Rerender with project B
    rerender(createElement(SequenceList, { projectId: "proj-b" }));

    await waitFor(() => {
      expect(screen.getByText("seq-for-proj-b-1")).toBeTruthy();
      expect(screen.getByText("seq-for-proj-b-2")).toBeTruthy();
    });

    // listSequences was called for both project IDs
    expect(listSequences).toHaveBeenCalledWith("proj-a");
    expect(listSequences).toHaveBeenCalledWith("proj-b");
  });
});

describe("SequenceList — ul role", () => {
  it("9. populated list renders a <ul> with role='list'", async () => {
    const sequences = [
      makeSequence({ name: "seq-x", description: "X." }),
    ];
    vi.mocked(listSequences).mockResolvedValue(sequences);
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "proj-1" }), { wrapper });
    await waitFor(() => {
      expect(screen.getByRole("list")).toBeTruthy();
    });
  });
});

describe("SequenceList — empty projectId disables query", () => {
  it("10. empty string projectId does not call listSequences", async () => {
    // The hook's `enabled: !!projectId` guard means listSequences must never be called
    // when projectId is "". The component will stay in its initial (no-data) state.
    // We verify by checking listSequences was never invoked.
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "" }), { wrapper });

    // Give TanStack Query a tick to potentially fire the query
    await new Promise((r) => setTimeout(r, 50));

    expect(listSequences).not.toHaveBeenCalled();
  });

  it("10b. empty string projectId shows empty state with directory hint (disabled query = no data)", async () => {
    const { wrapper } = makeWrapper();
    render(createElement(SequenceList, { projectId: "" }), { wrapper });

    // With no data and query disabled, the component should render the empty state
    await waitFor(() => {
      expect(screen.getByText("No sequences found. Add .md files to .claude/sequences/ in your project.")).toBeTruthy();
    });
  });
});

describe("SequenceList — selectedId resets on projectId change", () => {
  it("11. selecting a row then changing projectId resets selectedId", async () => {
    const seqsA = [
      makeSequence({ name: "seq-a", description: "For A." }),
    ];
    const seqsB = [
      makeSequence({ name: "seq-b", description: "For B." }),
    ];

    vi.mocked(listSequences)
      .mockResolvedValueOnce(seqsA)
      .mockResolvedValueOnce(seqsB);

    const { wrapper, queryClient } = makeWrapper();

    const { rerender } = render(createElement(SequenceList, { projectId: "proj-a" }), { wrapper });

    // Wait for seq-a to render
    await waitFor(() => {
      expect(screen.getAllByRole("listitem").length).toBe(1);
    });

    // Select the row
    const row = screen.getByRole("listitem");
    fireEvent.click(row);
    expect(row.getAttribute("data-selected")).toBe("true");

    // Clear cache so project B triggers a new fetch
    queryClient.clear();

    // Switch to project B
    rerender(createElement(SequenceList, { projectId: "proj-b" }));

    await waitFor(() => {
      expect(screen.getByText("seq-b")).toBeTruthy();
    });

    // The row for seq-b should not be selected (selectedId was reset)
    const rowB = screen.getByRole("listitem");
    expect(rowB.getAttribute("data-selected")).toBe("false");
  });
});
