import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
// Unit tests for useSettings hook. See docs/tasks/T1.4.md.
import { renderHook, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings, SettingsPatch } from "../ipc/bindings";

// ---------------------------------------------------------------------------
// Mocks — must be declared before dynamic imports
// ---------------------------------------------------------------------------

vi.mock("../ipc/commands", () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  openLogsFolder: vi.fn(),
}));

// Import after mock registration
import { getSettings, updateSettings } from "../ipc/commands";
import { SETTINGS_QUERY_KEY, useSettings } from "./useSettings";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const MOCK_SETTINGS: Settings = {
  parent_dir: null,
  claude_cli_path: null,
  git_poll_interval_secs: 10,
  usage_poll_interval_secs: 60,
  retention_days: 30,
  retention_size_mb: 500,
  view_mode: "Grid",
};

const MOCK_PATCH: SettingsPatch = {
  git_poll_interval_secs: 10,
  usage_poll_interval_secs: 60,
  retention_days: 30,
  retention_size_mb: 500,
  view_mode: "Grid",
  claude_cli_path: null,
  parent_dir: null,
};

// ---------------------------------------------------------------------------
// Helpers
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
// Tests
// ---------------------------------------------------------------------------

describe("useSettings hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls getSettings on mount", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    const { wrapper } = makeWrapper();

    renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(getSettings).toHaveBeenCalledOnce();
    });
  });

  it("returns settings data after successful fetch", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.settings).toEqual(MOCK_SETTINGS);
    expect(result.current.error).toBeNull();
  });

  it("exposes isLoading true initially", () => {
    // Never resolves — keeps hook in loading state
    vi.mocked(getSettings).mockImplementation(() => new Promise(() => {}));
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSettings(), { wrapper });

    expect(result.current.isLoading).toBe(true);
    expect(result.current.settings).toBeUndefined();
  });

  it("calls updateSettings when mutation fires", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    const updated: Settings = { ...MOCK_SETTINGS, git_poll_interval_secs: 30 };
    vi.mocked(updateSettings).mockResolvedValue(updated);

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await result.current.updateSettings(MOCK_PATCH);

    expect(updateSettings).toHaveBeenCalledOnce();
    expect(updateSettings).toHaveBeenCalledWith(MOCK_PATCH);
  });

  it("updates cached data after successful mutation", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    const updated: Settings = { ...MOCK_SETTINGS, git_poll_interval_secs: 30 };
    vi.mocked(updateSettings).mockResolvedValue(updated);

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.settings?.git_poll_interval_secs).toBe(10);

    await result.current.updateSettings(MOCK_PATCH);

    await waitFor(() => {
      expect(result.current.settings?.git_poll_interval_secs).toBe(30);
    });
  });

  it("exposes isSaving true while mutation is pending", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);

    let resolveUpdate!: (v: Settings) => void;
    vi.mocked(updateSettings).mockImplementation(
      () =>
        new Promise<Settings>((res) => {
          resolveUpdate = res;
        })
    );

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    // Start mutation without awaiting
    result.current.updateSettings(MOCK_PATCH);

    await waitFor(() => {
      expect(result.current.isSaving).toBe(true);
    });

    // Resolve and confirm isSaving drops
    resolveUpdate({ ...MOCK_SETTINGS });

    await waitFor(() => {
      expect(result.current.isSaving).toBe(false);
    });
  });

  it("exposes error when getSettings rejects", async () => {
    const fetchError = new Error("network error");
    vi.mocked(getSettings).mockRejectedValue(fetchError);
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.error).toBeTruthy();
    });

    expect(result.current.settings).toBeUndefined();
  });

  it("exports SETTINGS_QUERY_KEY as ['settings']", () => {
    expect(SETTINGS_QUERY_KEY).toEqual(["settings"]);
  });
});
