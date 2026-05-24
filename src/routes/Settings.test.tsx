// Integration tests for Settings route component. See docs/tasks/T1.4.md § Test scenarios.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createElement } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../ipc/bindings";

// ---------------------------------------------------------------------------
// Mocks — declared before any dynamic imports
// ---------------------------------------------------------------------------

vi.mock("../ipc/commands", () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  openLogsFolder: vi.fn(),
}));

// react-router-dom's useNavigate is used by Settings; we capture navigate calls.
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

// Import subjects AFTER mocks are registered.
import { getSettings, openLogsFolder, updateSettings } from "../ipc/commands";
import { useUiStore } from "../stores/ui";
import Settings from "./Settings";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const MOCK_SETTINGS: Settings = {
  parent_dir: "/home/user/projects",
  claude_cli_path: "/usr/local/bin/claude",
  git_poll_interval_secs: 10,
  usage_poll_interval_secs: 60,
  retention_days: 30,
  retention_size_mb: 500,
  view_mode: "Grid",
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  function Wrapper({ children }: { children: React.ReactNode }) {
    return createElement(
      QueryClientProvider,
      { client: queryClient },
      createElement(MemoryRouter, null, children)
    );
  }
  return { queryClient, Wrapper };
}

function renderSettings() {
  const { Wrapper } = makeWrapper();
  const user = userEvent.setup();
  const result = render(<Settings />, { wrapper: Wrapper });
  return { ...result, user };
}

// ---------------------------------------------------------------------------
// Reset between tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers({ shouldAdvanceTime: true });
  // Reset Zustand ui store to its default state.
  useUiStore.setState({ viewMode: "Grid" });
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// Test suites
// ---------------------------------------------------------------------------

describe("Settings screen — loading state", () => {
  it("shows loading indicator while settings are being fetched", () => {
    vi.mocked(getSettings).mockImplementation(() => new Promise(() => {}));
    renderSettings();
    expect(screen.getByText(/loading settings/i)).toBeTruthy();
  });
});

describe("Settings screen — renders all 7 fields", () => {
  beforeEach(() => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
  });

  it("renders the Settings heading", async () => {
    renderSettings();
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
  });

  it("renders Claude CLI path field with loaded value", async () => {
    renderSettings();
    const input = (await screen.findByLabelText(/claude cli path/i)) as HTMLInputElement;
    expect(input.value).toBe("/usr/local/bin/claude");
  });

  it("renders Projects parent directory field with loaded value", async () => {
    renderSettings();
    const input = (await screen.findByLabelText(/projects parent directory/i)) as HTMLInputElement;
    expect(input.value).toBe("/home/user/projects");
  });

  it("renders Git poll interval field", async () => {
    renderSettings();
    const input = (await screen.findByLabelText(/git poll interval/i)) as HTMLInputElement;
    expect(input.value).toBe("10");
  });

  it("renders Usage poll interval field", async () => {
    renderSettings();
    const input = (await screen.findByLabelText(/usage poll interval/i)) as HTMLInputElement;
    expect(input.value).toBe("60");
  });

  it("renders Retention days field", async () => {
    renderSettings();
    const input = (await screen.findByLabelText(/retention days/i)) as HTMLInputElement;
    expect(input.value).toBe("30");
  });

  it("renders Retention size field", async () => {
    renderSettings();
    const input = (await screen.findByLabelText(/retention size/i)) as HTMLInputElement;
    expect(input.value).toBe("500");
  });

  it("renders View mode toggle buttons (Grid and List)", async () => {
    renderSettings();
    // Wait for form to render
    await screen.findByRole("heading", { name: "Settings" });
    expect(screen.getByRole("button", { name: /^grid$/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /^list$/i })).toBeTruthy();
  });
});

describe("Settings screen — save happy path", () => {
  it("calls updateSettings with the correct patch on save", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    const updated: Settings = { ...MOCK_SETTINGS, git_poll_interval_secs: 20 };
    vi.mocked(updateSettings).mockResolvedValue(updated);

    const { user } = renderSettings();

    // Wait for form to initialise.
    const gitPollInput = await screen.findByLabelText(/git poll interval/i);

    // Change git poll to 20.
    await user.clear(gitPollInput);
    await user.type(gitPollInput, "20");

    const saveBtn = screen.getByRole("button", { name: /save settings/i });
    await user.click(saveBtn);

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledOnce();
    });

    const callArg = vi.mocked(updateSettings).mock.calls[0][0];
    expect(callArg.git_poll_interval_secs).toBe(20);
    expect(callArg.claude_cli_path).toBe("/usr/local/bin/claude");
    expect(callArg.parent_dir).toBe("/home/user/projects");
    expect(callArg.usage_poll_interval_secs).toBe(60);
    expect(callArg.retention_days).toBe(30);
    expect(callArg.retention_size_mb).toBe(500);
    expect(callArg.view_mode).toBe("Grid");
  });

  it("shows Saved! confirmation after successful save", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    vi.mocked(updateSettings).mockResolvedValue(MOCK_SETTINGS);

    const { user } = renderSettings();
    await screen.findByLabelText(/git poll interval/i);

    const saveBtn = screen.getByRole("button", { name: /save settings/i });
    await user.click(saveBtn);

    expect(await screen.findByText("Saved!")).toBeTruthy();
  });

  it("hides Saved! confirmation after 2 seconds", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    vi.mocked(updateSettings).mockResolvedValue(MOCK_SETTINGS);

    const { user } = renderSettings();
    await screen.findByLabelText(/git poll interval/i);

    const saveBtn = screen.getByRole("button", { name: /save settings/i });
    await user.click(saveBtn);

    expect(await screen.findByText("Saved!")).toBeTruthy();

    // Advance fake timers past the 2-second confirmation window.
    vi.advanceTimersByTime(2100);

    await waitFor(() => {
      expect(screen.queryByText("Saved!")).toBeNull();
    });
  });

  it("shows save error when updateSettings rejects", async () => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
    vi.mocked(updateSettings).mockRejectedValue(new Error("disk full"));

    const { user } = renderSettings();
    await screen.findByLabelText(/git poll interval/i);

    const saveBtn = screen.getByRole("button", { name: /save settings/i });
    await user.click(saveBtn);

    // The save error is rendered in a role="alert" span containing the message text.
    const alerts = await screen.findAllByRole("alert");
    const errorAlert = alerts.find((el) => el.textContent?.includes("disk full"));
    expect(errorAlert).toBeTruthy();
  });

  it("maps empty path field to null in the patch", async () => {
    vi.mocked(getSettings).mockResolvedValue({
      ...MOCK_SETTINGS,
      claude_cli_path: null,
      parent_dir: null,
    });
    vi.mocked(updateSettings).mockResolvedValue(MOCK_SETTINGS);

    const { user } = renderSettings();
    await screen.findByLabelText(/git poll interval/i);

    // Both path fields are empty; save without editing.
    const saveBtn = screen.getByRole("button", { name: /save settings/i });
    await user.click(saveBtn);

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledOnce();
    });

    const callArg = vi.mocked(updateSettings).mock.calls[0][0];
    expect(callArg.claude_cli_path).toBeNull();
    expect(callArg.parent_dir).toBeNull();
  });
});

describe("Settings screen — view mode toggle", () => {
  beforeEach(() => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
  });

  it("Grid button is aria-pressed=true when view mode is Grid", async () => {
    renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const gridBtn = screen.getByRole("button", { name: /^grid$/i });
    expect(gridBtn.getAttribute("aria-pressed")).toBe("true");
    const listBtn = screen.getByRole("button", { name: /^list$/i });
    expect(listBtn.getAttribute("aria-pressed")).toBe("false");
  });

  it("clicking List updates Zustand ui store immediately", async () => {
    const { user } = renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const listBtn = screen.getByRole("button", { name: /^list$/i });
    await user.click(listBtn);

    expect(useUiStore.getState().viewMode).toBe("List");
  });

  it("clicking Grid keeps Zustand ui store as Grid", async () => {
    const { user } = renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const gridBtn = screen.getByRole("button", { name: /^grid$/i });
    await user.click(gridBtn);

    expect(useUiStore.getState().viewMode).toBe("Grid");
  });

  it("view mode toggle is included in the save patch", async () => {
    vi.mocked(updateSettings).mockResolvedValue({ ...MOCK_SETTINGS, view_mode: "List" });
    const { user } = renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const listBtn = screen.getByRole("button", { name: /^list$/i });
    await user.click(listBtn);

    const saveBtn = screen.getByRole("button", { name: /save settings/i });
    await user.click(saveBtn);

    await waitFor(() => expect(updateSettings).toHaveBeenCalledOnce());
    const callArg = vi.mocked(updateSettings).mock.calls[0][0];
    expect(callArg.view_mode).toBe("List");
  });

  it("initialises view mode from Zustand store, not from backend response", async () => {
    // Backend says Grid, but store already has List set.
    useUiStore.setState({ viewMode: "List" });

    renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const listBtn = screen.getByRole("button", { name: /^list$/i });
    expect(listBtn.getAttribute("aria-pressed")).toBe("true");
  });
});

describe("Settings screen — unsaved-changes prompt", () => {
  beforeEach(() => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
  });

  it("navigates back without prompt when form is clean", async () => {
    const { user } = renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const backBtn = screen.getByRole("button", { name: /go back/i });
    await user.click(backBtn);

    expect(mockNavigate).toHaveBeenCalledWith(-1);
  });

  it("shows window.confirm when form is dirty and back is clicked", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    const { user } = renderSettings();
    const gitPollInput = await screen.findByLabelText(/git poll interval/i);

    // Make form dirty.
    await user.clear(gitPollInput);
    await user.type(gitPollInput, "15");

    const backBtn = screen.getByRole("button", { name: /go back/i });
    await user.click(backBtn);

    expect(confirmSpy).toHaveBeenCalledWith("You have unsaved changes. Leave without saving?");
    // User cancelled — navigate should NOT have been called.
    expect(mockNavigate).not.toHaveBeenCalled();

    confirmSpy.mockRestore();
  });

  it("navigates back when user confirms the unsaved-changes dialog", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    const { user } = renderSettings();
    const gitPollInput = await screen.findByLabelText(/git poll interval/i);

    await user.clear(gitPollInput);
    await user.type(gitPollInput, "15");

    const backBtn = screen.getByRole("button", { name: /go back/i });
    await user.click(backBtn);

    expect(confirmSpy).toHaveBeenCalled();
    expect(mockNavigate).toHaveBeenCalledWith(-1);

    confirmSpy.mockRestore();
  });
});

describe("Settings screen — numeric field validation", () => {
  beforeEach(() => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
  });

  // Git poll interval: must be 5–3600
  it("shows error and disables Save when git poll interval is below minimum (4)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/git poll interval/i);

    await user.clear(input);
    await user.type(input, "4");

    expect(await screen.findByText(/must be an integer between 5 and 3600/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  it("shows error and disables Save when git poll interval is above maximum (3601)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/git poll interval/i);

    await user.clear(input);
    await user.type(input, "3601");

    expect(await screen.findByText(/must be an integer between 5 and 3600/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  it("accepts the boundary value 5 for git poll interval", async () => {
    vi.mocked(updateSettings).mockResolvedValue(MOCK_SETTINGS);
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/git poll interval/i);

    await user.clear(input);
    await user.type(input, "5");

    expect(screen.queryByText(/must be an integer between 5 and 3600/i)).toBeNull();
    expect(screen.getByRole("button", { name: /save settings/i })).not.toBeDisabled();
  });

  // Usage poll interval: must be 30–3600
  it("shows error when usage poll interval is below minimum (29)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/usage poll interval/i);

    await user.clear(input);
    await user.type(input, "29");

    expect(await screen.findByText(/must be an integer between 30 and 3600/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  it("shows error when usage poll interval is above maximum (3601)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/usage poll interval/i);

    await user.clear(input);
    await user.type(input, "3601");

    expect(await screen.findByText(/must be an integer between 30 and 3600/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  // Retention days: must be 1–90
  it("shows error when retention days is below minimum (0)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/retention days/i);

    await user.clear(input);
    await user.type(input, "0");

    expect(await screen.findByText(/must be an integer between 1 and 90/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  it("shows error when retention days is above maximum (91)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/retention days/i);

    await user.clear(input);
    await user.type(input, "91");

    expect(await screen.findByText(/must be an integer between 1 and 90/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  // Retention size: must be 50–10240
  it("shows error when retention size is below minimum (49)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/retention size/i);

    await user.clear(input);
    await user.type(input, "49");

    expect(await screen.findByText(/must be an integer between 50 and 10240/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  it("shows error when retention size is above maximum (10241)", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/retention size/i);

    await user.clear(input);
    await user.type(input, "10241");

    expect(await screen.findByText(/must be an integer between 50 and 10240/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /save settings/i })).toBeDisabled();
  });

  it("clears validation error when field is corrected", async () => {
    const { user } = renderSettings();
    const input = await screen.findByLabelText(/git poll interval/i);

    await user.clear(input);
    await user.type(input, "4");
    expect(await screen.findByText(/must be an integer between 5 and 3600/i)).toBeTruthy();

    await user.clear(input);
    await user.type(input, "10");
    await waitFor(() => {
      expect(screen.queryByText(/must be an integer between 5 and 3600/i)).toBeNull();
    });
  });
});

describe("Settings screen — Open logs folder button", () => {
  beforeEach(() => {
    vi.mocked(getSettings).mockResolvedValue(MOCK_SETTINGS);
  });

  it("calls openLogsFolder when button is clicked", async () => {
    vi.mocked(openLogsFolder).mockResolvedValue(undefined);
    const { user } = renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const logsBtn = screen.getByRole("button", { name: /open logs folder/i });
    await user.click(logsBtn);

    await waitFor(() => {
      expect(openLogsFolder).toHaveBeenCalledOnce();
    });
  });

  it("shows Opening… text while openLogsFolder is in-flight", async () => {
    let resolveOpen!: () => void;
    vi.mocked(openLogsFolder).mockImplementation(
      () =>
        new Promise<void>((res) => {
          resolveOpen = res;
        })
    );

    const { user } = renderSettings();
    await screen.findByRole("heading", { name: "Settings" });

    const logsBtn = screen.getByRole("button", { name: /open logs folder/i });
    await user.click(logsBtn);

    expect(await screen.findByText(/opening…/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /open logs folder/i })).toBeDisabled();

    resolveOpen();
    await waitFor(() => {
      expect(screen.queryByText(/opening…/i)).toBeNull();
    });
  });
});
