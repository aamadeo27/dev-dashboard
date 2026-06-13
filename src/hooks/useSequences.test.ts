// Tests for useSequences hook. See docs/tasks/T3.2.md.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
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
import { SEQUENCES_QUERY_KEY, useSequences } from "./useSequences";

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

describe("useSequences — query key", () => {
  it("HK-01. SEQUENCES_QUERY_KEY returns ['sequences', projectId]", () => {
    expect(SEQUENCES_QUERY_KEY("proj-abc")).toEqual(["sequences", "proj-abc"]);
  });

  it("HK-02. SEQUENCES_QUERY_KEY is distinct for different projectIds", () => {
    expect(SEQUENCES_QUERY_KEY("proj-1")).not.toEqual(SEQUENCES_QUERY_KEY("proj-2"));
  });
});

describe("useSequences — enabled guard", () => {
  it("HK-03. does not call listSequences when projectId is empty string", async () => {
    // Never resolves — we just need to confirm it is not called
    vi.mocked(listSequences).mockImplementation(() => new Promise(() => {}));
    const { wrapper } = makeWrapper();

    renderHook(() => useSequences(""), { wrapper });

    // Give TanStack Query time to potentially fire
    await new Promise((r) => setTimeout(r, 50));

    expect(listSequences).not.toHaveBeenCalled();
  });

  it("HK-04. isLoading is false when projectId is empty string (query is disabled)", async () => {
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSequences(""), { wrapper });

    // A disabled query starts with isPending=true but isLoading=false in TanStack Query v5
    // because isLoading = isPending && isFetching, and a disabled query never fetches.
    await new Promise((r) => setTimeout(r, 50));
    expect(result.current.isLoading).toBe(false);
  });

  it("HK-05. calls listSequences when projectId is a non-empty string", async () => {
    const sequences = [makeSequence()];
    vi.mocked(listSequences).mockResolvedValue(sequences);
    const { wrapper } = makeWrapper();

    renderHook(() => useSequences("proj-1"), { wrapper });

    await waitFor(() => {
      expect(listSequences).toHaveBeenCalledWith("proj-1");
    });
  });
});

describe("useSequences — data shape", () => {
  it("HK-06. returns data as Sequence[] on success", async () => {
    const sequences = [
      makeSequence({ name: "deploy", description: "Deploys." }),
      makeSequence({ name: "test", description: "Tests." }),
    ];
    vi.mocked(listSequences).mockResolvedValue(sequences);
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSequences("proj-1"), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).toEqual(sequences);
    expect(result.current.error).toBeNull();
  });

  it("HK-07. exposes isLoading=true initially for a valid projectId", () => {
    vi.mocked(listSequences).mockImplementation(() => new Promise(() => {}));
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSequences("proj-1"), { wrapper });

    expect(result.current.isLoading).toBe(true);
    expect(result.current.data).toBeUndefined();
  });

  it("HK-08. exposes error when listSequences rejects", async () => {
    vi.mocked(listSequences).mockRejectedValue(new Error("NOT_FOUND: project not found"));
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSequences("proj-bad"), { wrapper });

    await waitFor(() => {
      expect(result.current.error).toBeTruthy();
    });

    expect(result.current.data).toBeUndefined();
    expect((result.current.error as Error).message).toBe("NOT_FOUND: project not found");
  });

  it("HK-09. returns empty array data for a project with no sequences", async () => {
    vi.mocked(listSequences).mockResolvedValue([]);
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSequences("proj-empty"), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).toEqual([]);
  });
});

describe("useSequences — cache isolation", () => {
  it("HK-10. different projectIds use independent cache entries", async () => {
    const seqsA = [makeSequence({ name: "seq-a" })];
    const seqsB = [makeSequence({ name: "seq-b" })];

    vi.mocked(listSequences).mockResolvedValueOnce(seqsA).mockResolvedValueOnce(seqsB);

    const { wrapper, queryClient } = makeWrapper();

    // Fetch for proj-a
    const { result: resultA } = renderHook(() => useSequences("proj-a"), { wrapper });
    await waitFor(() => expect(resultA.current.isLoading).toBe(false));

    // Clear cache to ensure proj-b makes a fresh call
    queryClient.clear();

    // Fetch for proj-b in a second hook instance
    const { result: resultB } = renderHook(() => useSequences("proj-b"), { wrapper });
    await waitFor(() => expect(resultB.current.isLoading).toBe(false));

    expect(listSequences).toHaveBeenCalledWith("proj-a");
    expect(listSequences).toHaveBeenCalledWith("proj-b");
    expect(listSequences).toHaveBeenCalledTimes(2);
  });
});
