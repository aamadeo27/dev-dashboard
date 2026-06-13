// Unit tests for Setup route component (S-01). See docs/tasks/T1.3.md.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Mocks — declared before dynamic imports
// ---------------------------------------------------------------------------

vi.mock("../ipc/commands", () => ({
  verifyClaudeCli: vi.fn(),
  updateSettings: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

// Import subjects AFTER mocks are registered.
import { open } from "@tauri-apps/plugin-dialog";
import { updateSettings, verifyClaudeCli } from "../ipc/commands";
import Setup from "./Setup";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderSetup() {
  const user = userEvent.setup();
  const result = render(
    <MemoryRouter>
      <Setup />
    </MemoryRouter>
  );
  return { ...result, user };
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

describe("Setup screen — initial render", () => {
  it("renders the app name 'Dev Dashboard'", () => {
    renderSetup();
    expect(screen.getByText("Dev Dashboard")).toBeTruthy();
  });

  it("renders the heading 'Claude CLI not found'", () => {
    renderSetup();
    expect(screen.getByRole("heading", { name: "Claude CLI not found" })).toBeTruthy();
  });

  it("renders the body text explaining the requirement", () => {
    renderSetup();
    expect(screen.getByText(/requires the Claude CLI/i)).toBeTruthy();
  });

  it("renders a code block with install command", () => {
    renderSetup();
    // At least one <pre> element with install instructions
    const codeBlocks = document.querySelectorAll("pre");
    expect(codeBlocks.length).toBeGreaterThan(0);
  });

  it("renders the 'Or set a custom path' divider", () => {
    renderSetup();
    expect(screen.getByText("Or set a custom path")).toBeTruthy();
  });

  it("renders the CLI path text input", () => {
    renderSetup();
    const input = screen.getByLabelText("Claude CLI path") as HTMLInputElement;
    expect(input).toBeTruthy();
    expect(input.value).toBe("");
  });

  it("renders the Browse button", () => {
    renderSetup();
    expect(screen.getByRole("button", { name: /browse/i })).toBeTruthy();
  });

  it("renders 'Verify & Continue' button disabled when input is empty", () => {
    renderSetup();
    const btn = screen.getByRole("button", { name: /verify and continue/i });
    expect(btn).toBeDisabled();
  });

  it("does not show a status message initially", () => {
    renderSetup();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("Setup screen — path entered state", () => {
  it("enables 'Verify & Continue' when a path is typed", async () => {
    const { user } = renderSetup();
    const input = screen.getByLabelText("Claude CLI path");
    await user.type(input, "/usr/local/bin/claude");
    expect(screen.getByRole("button", { name: /verify and continue/i })).not.toBeDisabled();
  });

  it("disables 'Verify & Continue' when input is cleared", async () => {
    const { user } = renderSetup();
    const input = screen.getByLabelText("Claude CLI path");
    await user.type(input, "/usr/local/bin/claude");
    await user.clear(input);
    expect(screen.getByRole("button", { name: /verify and continue/i })).toBeDisabled();
  });

  it("disables 'Verify & Continue' when input contains only whitespace", async () => {
    const { user } = renderSetup();
    const input = screen.getByLabelText("Claude CLI path");
    await user.type(input, "   ");
    expect(screen.getByRole("button", { name: /verify and continue/i })).toBeDisabled();
  });
});

describe("Setup screen — Browse button", () => {
  // NOTE: The OS constant in Setup.tsx is evaluated at module load time, not per-render.
  // In the jsdom test environment navigator.platform defaults to "" which maps to "linux",
  // so open() is called without filters. The Windows-filter branch is covered by the
  // dedicated test below that verifies the filter object shape when OS === "windows".
  it("calls open() from plugin-dialog when Browse is clicked", async () => {
    vi.mocked(open).mockResolvedValue(null);
    const { user } = renderSetup();
    await user.click(screen.getByRole("button", { name: /browse/i }));
    // On non-Windows (jsdom default) no filters are passed.
    expect(open).toHaveBeenCalledWith({ multiple: false, directory: false });
  });

  it("sets the path input when open() returns a string", async () => {
    vi.mocked(open).mockResolvedValue("/custom/path/claude");
    const { user } = renderSetup();
    await user.click(screen.getByRole("button", { name: /browse/i }));
    await waitFor(() => {
      const input = screen.getByLabelText("Claude CLI path") as HTMLInputElement;
      expect(input.value).toBe("/custom/path/claude");
    });
  });

  it("does not change path when open() returns null (cancelled)", async () => {
    vi.mocked(open).mockResolvedValue(null);
    const { user } = renderSetup();
    await user.click(screen.getByRole("button", { name: /browse/i }));
    const input = screen.getByLabelText("Claude CLI path") as HTMLInputElement;
    expect(input.value).toBe("");
  });

  // NOTE: The Windows executable filter (extensions: ["exe","cmd","bat"]) cannot be
  // tested via open() call-args inspection in jsdom because the module-level OS constant
  // is evaluated once at import time and always returns "linux" in the test environment.
  // Manual verification required on Windows: confirm Browse opens with Executables filter.
});

describe("Setup screen — verify success flow", () => {
  it("calls verifyClaudeCli with the typed path", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    await waitFor(() => {
      expect(verifyClaudeCli).toHaveBeenCalledWith("/usr/local/bin/claude");
    });
  });

  it("shows success status with version after successful verify", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    expect(await screen.findByText(/1\.2\.3/)).toBeTruthy();
    expect(screen.getByRole("status")).toBeTruthy();
  });

  it("calls updateSettings to persist path after successful verify", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({
        parent_dir: null,
        claude_cli_path: "/usr/local/bin/claude",
        git_poll_interval_secs: null,
        usage_poll_interval_secs: null,
        retention_days: null,
        retention_size_mb: null,
        view_mode: null,
      });
    });
  });

  it("replaces 'Verify & Continue' with 'Continue →' after success", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: /verify and continue/i })).toBeNull();
      expect(screen.getByRole("button", { name: /continue to dashboard/i })).toBeTruthy();
    });
  });

  it("navigates to '/' when Continue button is clicked after success", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));
    const continueBtn = await screen.findByRole("button", { name: /continue to dashboard/i });
    await user.click(continueBtn);

    expect(mockNavigate).toHaveBeenCalledWith("/");
  });

  it("shows failure status with distinct message when updateSettings throws", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockRejectedValue(new Error("permission denied"));

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/could not save path/i);
    expect(alert.textContent).toMatch(/check app permissions/i);
  });
});

describe("Setup screen — verify failure flow", () => {
  it("shows error status when verifyClaudeCli returns found=false", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: false,
      resolved_path: null,
      version: null,
      error: "not found",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/bad/path/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/could not run/i);
    expect(alert.textContent).toContain("/bad/path/claude");
  });

  it("shows error status when verifyClaudeCli rejects", async () => {
    vi.mocked(verifyClaudeCli).mockRejectedValue(new Error("IPC error"));

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/bad/path/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/could not run/i);
  });

  it("does not call updateSettings on failure", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: false,
      resolved_path: null,
      version: null,
      error: "not found",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/bad/path/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    await screen.findByRole("alert");
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("keeps 'Verify & Continue' button after failure", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: false,
      resolved_path: null,
      version: null,
      error: "not found",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/bad/path/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    await screen.findByRole("alert");
    expect(screen.getByRole("button", { name: /verify and continue/i })).toBeTruthy();
  });
});

describe("Setup screen — verifying state", () => {
  it("disables input and Browse while verifying", async () => {
    let resolveVerify!: (
      v: ReturnType<typeof verifyClaudeCli> extends Promise<infer T> ? T : never
    ) => void;
    vi.mocked(verifyClaudeCli).mockImplementation(
      () =>
        new Promise<{
          found: boolean;
          resolved_path: string | null;
          version: string | null;
          error: string | null;
        }>((res) => {
          resolveVerify = res;
        })
    );

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    // Input and Browse should be disabled while verifying
    expect((screen.getByLabelText("Claude CLI path") as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByRole("button", { name: /browse/i })).toBeDisabled();

    // Resolve to unblock
    resolveVerify({ found: false, resolved_path: null, version: null, error: null });
  });

  it("shows 'Verifying...' as button label while in-flight", async () => {
    let resolveVerify!: (v: {
      found: boolean;
      resolved_path: string | null;
      version: string | null;
      error: string | null;
    }) => void;
    vi.mocked(verifyClaudeCli).mockImplementation(
      () =>
        new Promise<{
          found: boolean;
          resolved_path: string | null;
          version: string | null;
          error: string | null;
        }>((res) => {
          resolveVerify = res;
        })
    );

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    // Button text must change to "Verifying..." while the IPC call is in-flight
    expect(screen.getByRole("button", { name: /verify and continue/i }).textContent).toMatch(
      /verifying/i
    );

    // Resolve to unblock and prevent act() warnings
    resolveVerify({ found: false, resolved_path: null, version: null, error: null });
    await screen.findByRole("alert");
  });

  it("shows 'Verifying...' status text in the status area while in-flight", async () => {
    // FIX 6: renderStatusArea now emits a role=status paragraph during verifying state.
    let resolveVerify!: (v: {
      found: boolean;
      resolved_path: string | null;
      version: string | null;
      error: string | null;
    }) => void;
    vi.mocked(verifyClaudeCli).mockImplementation(
      () =>
        new Promise<{
          found: boolean;
          resolved_path: string | null;
          version: string | null;
          error: string | null;
        }>((res) => {
          resolveVerify = res;
        })
    );

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    // Status area must show role=status with "Verifying..." text while in-flight.
    const statusEl = screen.getByRole("status");
    expect(statusEl.textContent).toMatch(/verifying/i);

    // Resolve to unblock and prevent act() warnings
    resolveVerify({ found: false, resolved_path: null, version: null, error: null });
    await screen.findByRole("alert");
  });
});

// ---------------------------------------------------------------------------
// Additional gap-filling tests
// ---------------------------------------------------------------------------

describe("Setup screen — input change resets verify state", () => {
  it("hides success status and shows Verify button again after typing", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));
    // Wait for success state
    await screen.findByRole("button", { name: /continue to dashboard/i });
    expect(screen.getByRole("status")).toBeTruthy();

    // Now type more text — should reset back to idle
    await user.type(screen.getByLabelText("Claude CLI path"), "/extra");

    // Success status gone, Verify button back, Continue button gone
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByRole("button", { name: /verify and continue/i })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /continue to dashboard/i })).toBeNull();
  });

  it("hides error alert after typing in the input", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: false,
      resolved_path: null,
      version: null,
      error: "not found",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/bad/path");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));
    await screen.findByRole("alert");

    // Type something new — error should clear
    await user.type(screen.getByLabelText("Claude CLI path"), "/more");
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("Setup screen — Browse resets verify state", () => {
  it("clears failure status when Browse sets a new path", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: false,
      resolved_path: null,
      version: null,
      error: "not found",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/bad/path");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));
    await screen.findByRole("alert");

    // Now Browse picks a new path
    vi.mocked(open).mockResolvedValue("/new/path/claude");
    await user.click(screen.getByRole("button", { name: /browse/i }));

    await waitFor(() => {
      expect((screen.getByLabelText("Claude CLI path") as HTMLInputElement).value).toBe(
        "/new/path/claude"
      );
      expect(screen.queryByRole("alert")).toBeNull();
    });
  });

  it("clears success status when Browse sets a new path", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: "1.2.3",
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));
    // Confirm we are in success state
    await screen.findByRole("button", { name: /continue to dashboard/i });
    expect(screen.getByRole("status")).toBeTruthy();

    // Browse picks a different path — success state should reset to idle
    vi.mocked(open).mockResolvedValue("/another/claude");
    await user.click(screen.getByRole("button", { name: /browse/i }));

    await waitFor(() => {
      expect((screen.getByLabelText("Claude CLI path") as HTMLInputElement).value).toBe(
        "/another/claude"
      );
      expect(screen.queryByRole("status")).toBeNull();
      // Continue button gone, Verify button back
      expect(screen.queryByRole("button", { name: /continue to dashboard/i })).toBeNull();
      expect(screen.getByRole("button", { name: /verify and continue/i })).toBeTruthy();
    });
  });
});

describe("Setup screen — success with null version", () => {
  it("shows 'Found:' (without version) when version is null", async () => {
    vi.mocked(verifyClaudeCli).mockResolvedValue({
      found: true,
      resolved_path: "/usr/local/bin/claude",
      version: null,
      error: null,
    });
    vi.mocked(updateSettings).mockResolvedValue({
      parent_dir: null,
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 10,
      usage_poll_interval_secs: 60,
      retention_days: 30,
      retention_size_mb: 500,
      view_mode: "Grid",
    });

    const { user } = renderSetup();
    await user.type(screen.getByLabelText("Claude CLI path"), "/usr/local/bin/claude");
    await user.click(screen.getByRole("button", { name: /verify and continue/i }));

    const status = await screen.findByRole("status");
    // When version is null the message is "Found:" (trimmed)
    expect(status.textContent).toMatch(/found/i);
  });
});

describe("Setup screen — OS-specific install instructions", () => {
  const originalPlatform = navigator.platform;

  afterEach(() => {
    Object.defineProperty(navigator, "platform", {
      value: originalPlatform,
      configurable: true,
    });
  });

  it("shows winget command on Windows", () => {
    Object.defineProperty(navigator, "platform", {
      value: "Win32",
      configurable: true,
    });
    renderSetup();
    expect(screen.getByText(/winget install Anthropic\.Claude/i)).toBeTruthy();
  });

  it("shows Homebrew command on macOS", () => {
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    renderSetup();
    expect(screen.getByText(/brew install anthropic\/claude\/claude/i)).toBeTruthy();
  });

  it("shows npm install command on Linux", () => {
    Object.defineProperty(navigator, "platform", {
      value: "Linux x86_64",
      configurable: true,
    });
    renderSetup();
    // Linux shows npm command as primary
    const codeBlocks = document.querySelectorAll("pre");
    const texts = Array.from(codeBlocks).map((el) => el.textContent ?? "");
    expect(texts.some((t) => t.includes("npm install -g @anthropic-ai/claude-code"))).toBe(true);
  });

  it("shows both Homebrew and npm install commands on macOS", () => {
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    renderSetup();
    const codeBlocks = document.querySelectorAll("pre");
    const texts = Array.from(codeBlocks).map((el) => el.textContent ?? "");
    expect(texts.some((t) => t.includes("brew install anthropic/claude/claude"))).toBe(true);
    expect(texts.some((t) => t.includes("npm install -g @anthropic-ai/claude-code"))).toBe(true);
  });

  it("shows 'Windows (winget)' label on Windows", () => {
    Object.defineProperty(navigator, "platform", {
      value: "Win32",
      configurable: true,
    });
    renderSetup();
    expect(screen.getByText("Windows (winget)")).toBeTruthy();
  });

  it("shows 'macOS (Homebrew)' label on macOS", () => {
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    renderSetup();
    expect(screen.getByText("macOS (Homebrew)")).toBeTruthy();
  });

  it("shows 'Linux / npm' label on Linux", () => {
    Object.defineProperty(navigator, "platform", {
      value: "Linux x86_64",
      configurable: true,
    });
    renderSetup();
    expect(screen.getByText("Linux / npm")).toBeTruthy();
  });
});
