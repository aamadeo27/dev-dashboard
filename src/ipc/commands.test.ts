import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Frontend IPC wrapper tests (T1.1: getSettings / updateSettings / openLogsFolder,
//                              T1.2: verifyClaudeCli,
//                              T2.1: listProjects / addProject / removeProject /
//                                    relocateProject / setProjectTags,
//                              T2.8: openInEditor / openInTerminal,
//                              T3.1: listSequences / refreshSequences)
//
// Tauri commands cannot be invoked in Vitest without a running Tauri window.
// These tests verify:
//   T1.1 — getSettings / updateSettings / openLogsFolder:
//     1. Each function is exported from commands.ts.
//     2. Each calls `invoke` with the correct command name.
//     3. `updateSettings` passes the patch object under the `patch` key.
//     4. `getSettings` and `openLogsFolder` pass no extra arguments.
//     5. The return value is the promise returned by `invoke`.
//   T1.2 — verifyClaudeCli:
//     1. Function is exported.
//     2. Called with undefined / no argument → invokes with { pathOverride: null }.
//     3. Called with a path string → invokes with { pathOverride: "<path>" }.
//     4. Returns the CliCheck resolved by invoke.
//     5. Propagates invoke rejection to the caller.
//   T2.1 — listProjects / addProject / removeProject / relocateProject / setProjectTags:
//     1. Each function is exported.
//     2. Each calls `invoke` with the correct snake_case command name.
//     3. Arguments are forwarded under the correct keys.
//     4. Return values / promise rejections propagate correctly.
//     5. renameProject is NOT exported (internal Rust command only).
//   T2.8 — openInEditor / openInTerminal:
//     1. Each function is exported.
//     2. Each calls `invoke` with the correct snake_case command name.
//     3. The id argument is forwarded under the `id` key.
//     4. Both return Promise<void>.
//     5. Both propagate invoke rejection to the caller.
//
// Actual Tauri invocation is mocked; no Tauri runtime is required.
// ---------------------------------------------------------------------------

// Mock @tauri-apps/api/core before importing the module under test so that
// `invoke` is replaced with a spy throughout the test suite.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Import after mock registration.
import { invoke } from "@tauri-apps/api/core";
import type {
  CliCheck,
  LaunchInput,
  Project,
  Run,
  RunStatus,
  Sequence,
  Settings,
  SettingsPatch,
} from "./bindings";
import * as allCommands from "./commands";
import {
  addProject,
  getSettings,
  launchRun,
  listProjects,
  listSequences,
  openInEditor,
  openInTerminal,
  openLogsFolder,
  refreshSequences,
  relocateProject,
  removeProject,
  sendInput,
  setProjectTags,
  stopRun,
  updateSettings,
  verifyClaudeCli,
} from "./commands";

// Minimal Settings fixture that satisfies the TS type.
const MOCK_SETTINGS: Settings = {
  parent_dir: null,
  claude_cli_path: null,
  git_poll_interval_secs: 10,
  usage_poll_interval_secs: 60,
  retention_days: 30,
  retention_size_mb: 500,
  view_mode: "Grid",
};

describe("getSettings IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof getSettings).toBe("function");
  });

  it("invokes the command named 'get_settings'", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_SETTINGS);

    await getSettings();

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("get_settings");
  });

  it("does not pass extra arguments to invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_SETTINGS);

    await getSettings();

    const [, secondArg] = vi.mocked(invoke).mock.calls[0] ?? [];
    expect(secondArg).toBeUndefined();
  });

  it("returns the Settings object resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_SETTINGS);

    const result = await getSettings();

    expect(result).toBe(MOCK_SETTINGS);
  });

  it("propagates invoke rejection to the caller", async () => {
    const error = { code: "IO", message: "disk full", details: null };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(getSettings()).rejects.toEqual(error);
  });
});

describe("updateSettings IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof updateSettings).toBe("function");
  });

  it("invokes the command named 'update_settings'", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_SETTINGS);
    const patch: SettingsPatch = {
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: null,
      usage_poll_interval_secs: null,
      retention_days: null,
      retention_size_mb: null,
      view_mode: null,
    };

    await updateSettings(patch);

    expect(invoke).toHaveBeenCalledOnce();
    const [commandName] = vi.mocked(invoke).mock.calls[0]!;
    expect(commandName).toBe("update_settings");
  });

  it("passes the patch under the 'patch' key", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_SETTINGS);
    const patch: SettingsPatch = {
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: 30,
      usage_poll_interval_secs: null,
      retention_days: null,
      retention_size_mb: null,
      view_mode: null,
    };

    await updateSettings(patch);

    expect(invoke).toHaveBeenCalledWith("update_settings", { patch });
  });

  it("passes an all-null patch verbatim", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_SETTINGS);
    const patch: SettingsPatch = {
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: null,
      usage_poll_interval_secs: null,
      retention_days: null,
      retention_size_mb: null,
      view_mode: null,
    };

    await updateSettings(patch);

    expect(invoke).toHaveBeenCalledWith("update_settings", { patch });
  });

  it("returns the updated Settings object resolved by invoke", async () => {
    const updated: Settings = { ...MOCK_SETTINGS, git_poll_interval_secs: 30 };
    vi.mocked(invoke).mockResolvedValue(updated);
    const patch: SettingsPatch = {
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: 30,
      usage_poll_interval_secs: null,
      retention_days: null,
      retention_size_mb: null,
      view_mode: null,
    };

    const result = await updateSettings(patch);

    expect(result).toBe(updated);
    expect(result.git_poll_interval_secs).toBe(30);
  });

  it("propagates invoke rejection to the caller (e.g. validation error)", async () => {
    const error = {
      code: "INVALID_INPUT",
      message: "Invalid input: git_poll_interval_secs must be between 5 and 3600, got 2",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);
    const patch: SettingsPatch = {
      parent_dir: null,
      claude_cli_path: null,
      git_poll_interval_secs: 2,
      usage_poll_interval_secs: null,
      retention_days: null,
      retention_size_mb: null,
      view_mode: null,
    };

    await expect(updateSettings(patch)).rejects.toEqual(error);
  });

  it("passes a full non-null patch verbatim", async () => {
    const fullSettings: Settings = {
      parent_dir: "/home/user/projects",
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 300,
      usage_poll_interval_secs: 120,
      retention_days: 7,
      retention_size_mb: 200,
      view_mode: "List",
    };
    vi.mocked(invoke).mockResolvedValue(fullSettings);

    const patch: SettingsPatch = {
      parent_dir: "/home/user/projects",
      claude_cli_path: "/usr/local/bin/claude",
      git_poll_interval_secs: 300,
      usage_poll_interval_secs: 120,
      retention_days: 7,
      retention_size_mb: 200,
      view_mode: "List",
    };

    await updateSettings(patch);

    expect(invoke).toHaveBeenCalledWith("update_settings", { patch });
  });
});

describe("openLogsFolder IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof openLogsFolder).toBe("function");
  });

  it("invokes the command named 'open_logs_folder'", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openLogsFolder();

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("open_logs_folder");
  });

  it("does not pass extra arguments to invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openLogsFolder();

    const [, secondArg] = vi.mocked(invoke).mock.calls[0] ?? [];
    expect(secondArg).toBeUndefined();
  });

  it("returns a Promise<void> resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await openLogsFolder();

    expect(result).toBeUndefined();
  });

  it("propagates invoke rejection to the caller", async () => {
    const error = { code: "IO", message: "I/O error: permission denied", details: null };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(openLogsFolder()).rejects.toEqual(error);
  });
});

const MOCK_CLI_CHECK_FOUND: CliCheck = {
  found: true,
  resolved_path: "/usr/local/bin/claude",
  version: "Claude Code 1.0.0",
  error: null,
};

const MOCK_CLI_CHECK_NOT_FOUND: CliCheck = {
  found: false,
  resolved_path: null,
  version: null,
  error: "Failed to launch Claude CLI at 'claude': No such file or directory",
};

describe("verifyClaudeCli IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof verifyClaudeCli).toBe("function");
  });

  it("invokes 'verify_claude_cli' with { pathOverride: null } when called with undefined", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_CLI_CHECK_FOUND);

    await verifyClaudeCli(undefined);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("verify_claude_cli", { pathOverride: null });
  });

  it("invokes 'verify_claude_cli' with { pathOverride: null } when called with no argument", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_CLI_CHECK_FOUND);

    await verifyClaudeCli();

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("verify_claude_cli", { pathOverride: null });
  });

  it("invokes 'verify_claude_cli' with the given path string under pathOverride", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_CLI_CHECK_FOUND);

    await verifyClaudeCli("/path/to/claude");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("verify_claude_cli", {
      pathOverride: "/path/to/claude",
    });
  });

  it("returns the CliCheck resolved by invoke when CLI is found", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_CLI_CHECK_FOUND);

    const result = await verifyClaudeCli();

    expect(result).toBe(MOCK_CLI_CHECK_FOUND);
    expect(result.found).toBe(true);
    expect(result.version).toBe("Claude Code 1.0.0");
  });

  it("returns the CliCheck resolved by invoke when CLI is not found", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_CLI_CHECK_NOT_FOUND);

    const result = await verifyClaudeCli();

    expect(result).toBe(MOCK_CLI_CHECK_NOT_FOUND);
    expect(result.found).toBe(false);
    expect(result.error).toBeTruthy();
  });

  it("propagates invoke rejection to the caller", async () => {
    const error = { code: "IO", message: "unexpected IPC error", details: null };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(verifyClaudeCli()).rejects.toEqual(error);
  });
});

// ---------------------------------------------------------------------------
// T2.1 project command wrappers
// ---------------------------------------------------------------------------

const MOCK_PROJECT: Project = {
  id: "019500000000000000000000001",
  name: "my-app",
  path: "/home/user/my-app",
  tags: [],
  language: null,
  package_manager: null,
  added_at: "2026-05-21T10:00:00Z",
  last_modified: null,
  is_missing: false,
};

describe("listProjects IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof listProjects).toBe("function");
  });

  it("invokes the command named 'list_projects' with no arguments", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_PROJECT]);

    await listProjects();

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("list_projects");
  });

  it("returns the Project array resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_PROJECT]);

    const result = await listProjects();

    expect(result).toHaveLength(1);
    expect(result[0]).toBe(MOCK_PROJECT);
  });

  it("returns an empty array when no projects are registered", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    const result = await listProjects();

    expect(result).toEqual([]);
  });

  it("propagates invoke rejection to the caller", async () => {
    const error = { code: "IO", message: "disk error", details: null };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(listProjects()).rejects.toEqual(error);
  });
});

describe("addProject IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof addProject).toBe("function");
  });

  it("invokes 'add_project' with the path under the 'path' key", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_PROJECT);

    await addProject("/home/user/my-app");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("add_project", { path: "/home/user/my-app" });
  });

  it("returns the Project resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_PROJECT);

    const result = await addProject("/home/user/my-app");

    expect(result).toBe(MOCK_PROJECT);
  });

  it("propagates ALREADY_EXISTS rejection to the caller", async () => {
    const error = {
      code: "ALREADY_EXISTS",
      message: "Already exists: project already registered at /home/user/my-app",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(addProject("/home/user/my-app")).rejects.toEqual(error);
  });
});

describe("removeProject IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof removeProject).toBe("function");
  });

  it("invokes 'remove_project' with the id under the 'id' key", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await removeProject("019500000000000000000000001");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("remove_project", { id: "019500000000000000000000001" });
  });

  it("returns void (undefined) when removal succeeds", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await removeProject("019500000000000000000000001");

    expect(result).toBeUndefined();
  });

  it("propagates NOT_FOUND rejection to the caller", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-id",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(removeProject("no-such-id")).rejects.toEqual(error);
  });
});

describe("relocateProject IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof relocateProject).toBe("function");
  });

  it("invokes 'relocate_project' with id and newPath under the correct keys", async () => {
    const updated = { ...MOCK_PROJECT, path: "/home/user/new-location" };
    vi.mocked(invoke).mockResolvedValue(updated);

    await relocateProject("019500000000000000000000001", "/home/user/new-location");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("relocate_project", {
      id: "019500000000000000000000001",
      newPath: "/home/user/new-location",
    });
  });

  it("returns the updated Project resolved by invoke", async () => {
    const updated = { ...MOCK_PROJECT, path: "/home/user/new-location" };
    vi.mocked(invoke).mockResolvedValue(updated);

    const result = await relocateProject("019500000000000000000000001", "/home/user/new-location");

    expect(result.path).toBe("/home/user/new-location");
  });

  it("propagates NOT_FOUND rejection to the caller", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-id",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(relocateProject("no-such-id", "/some/path")).rejects.toEqual(error);
  });

  it("propagates ALREADY_EXISTS rejection to the caller", async () => {
    const error = {
      code: "ALREADY_EXISTS",
      message: "Already exists: path already registered: /home/user/other-app",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(
      relocateProject("019500000000000000000000001", "/home/user/other-app")
    ).rejects.toEqual(error);
  });
});

describe("setProjectTags IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof setProjectTags).toBe("function");
  });

  it("invokes 'set_project_tags' with id and tags under the correct keys", async () => {
    const updated = { ...MOCK_PROJECT, tags: ["rust", "typescript"] };
    vi.mocked(invoke).mockResolvedValue(updated);

    await setProjectTags("019500000000000000000000001", ["Rust", "TypeScript"]);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("set_project_tags", {
      id: "019500000000000000000000001",
      tags: ["Rust", "TypeScript"],
    });
  });

  it("returns the updated Project with normalized tags resolved by invoke", async () => {
    const updated = { ...MOCK_PROJECT, tags: ["rust", "typescript"] };
    vi.mocked(invoke).mockResolvedValue(updated);

    const result = await setProjectTags("019500000000000000000000001", ["Rust", "TypeScript"]);

    expect(result.tags).toEqual(["rust", "typescript"]);
  });

  it("passes an empty tags array verbatim", async () => {
    const updated = { ...MOCK_PROJECT, tags: [] };
    vi.mocked(invoke).mockResolvedValue(updated);

    await setProjectTags("019500000000000000000000001", []);

    expect(invoke).toHaveBeenCalledWith("set_project_tags", {
      id: "019500000000000000000000001",
      tags: [],
    });
  });

  it("propagates NOT_FOUND rejection to the caller", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-id",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(setProjectTags("no-such-id", ["tag"])).rejects.toEqual(error);
  });
});

describe("renameProject is NOT exported from commands.ts", () => {
  it("renameProject is not a named export (internal Rust command only per KB §5.1)", () => {
    // Check the actual module namespace — this will catch a real export of renameProject.
    expect("renameProject" in allCommands).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// T2.8 — openInEditor / openInTerminal wrappers
// ---------------------------------------------------------------------------

describe("openInEditor IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof openInEditor).toBe("function");
  });

  it("invokes 'open_in_editor' with the id under the 'id' key", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openInEditor("019500000000000000000000001");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("open_in_editor", {
      id: "019500000000000000000000001",
    });
  });

  it("returns Promise<void> (undefined) on success", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await openInEditor("019500000000000000000000001");

    expect(result).toBeUndefined();
  });

  it("propagates NOT_FOUND rejection to the caller", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-id",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(openInEditor("no-such-id")).rejects.toEqual(error);
  });

  it("propagates IO rejection to the caller", async () => {
    const error = {
      code: "IO",
      message: "I/O error: No such file or directory",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(openInEditor("019500000000000000000000001")).rejects.toEqual(error);
  });

  it("forwards the id argument verbatim (does not transform it)", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const rawId = "  spaced-id  ";

    await openInEditor(rawId);

    expect(invoke).toHaveBeenCalledWith("open_in_editor", { id: rawId });
  });
});

describe("openInTerminal IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof openInTerminal).toBe("function");
  });

  it("invokes 'open_in_terminal' with the id under the 'id' key", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openInTerminal("019500000000000000000000001");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("open_in_terminal", {
      id: "019500000000000000000000001",
    });
  });

  it("returns Promise<void> (undefined) on success", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await openInTerminal("019500000000000000000000001");

    expect(result).toBeUndefined();
  });

  it("propagates NOT_FOUND rejection to the caller", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-id",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(openInTerminal("no-such-id")).rejects.toEqual(error);
  });

  it("propagates IO rejection to the caller", async () => {
    const error = {
      code: "IO",
      message: "I/O error: permission denied",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(openInTerminal("019500000000000000000000001")).rejects.toEqual(error);
  });

  it("forwards the id argument verbatim (does not transform it)", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const rawId = "  spaced-id  ";

    await openInTerminal(rawId);

    expect(invoke).toHaveBeenCalledWith("open_in_terminal", { id: rawId });
  });
});

// ---------------------------------------------------------------------------
// T2.8 gap tests — argument shape and return type exactness
// ---------------------------------------------------------------------------

describe("openInEditor argument shape (T2.8 gap)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("passes the id under the key 'id', not 'projectId'", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openInEditor("some-id");

    const [, args] = vi.mocked(invoke).mock.calls[0]!;
    // Must have the 'id' key
    expect(args).toHaveProperty("id", "some-id");
    // Must NOT have a 'projectId' key
    expect(args).not.toHaveProperty("projectId");
  });

  it("passes a single options object as the second arg (not a positional string)", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openInEditor("proj-123");

    const callArgs = vi.mocked(invoke).mock.calls[0]!;
    // Second argument must be a plain object, not a bare string
    expect(typeof callArgs[1]).toBe("object");
    expect(callArgs[1]).not.toBeNull();
    // And there must be no third argument
    expect(callArgs[2]).toBeUndefined();
  });

  it("resolved value is strictly undefined, not a Project or other object", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await openInEditor("proj-123");

    expect(result).toBeUndefined();
    // Confirm it is not an object (guards against Promise<Project> return type)
    expect(typeof result).not.toBe("object");
  });
});

describe("openInTerminal argument shape (T2.8 gap)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("passes the id under the key 'id', not 'projectId'", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openInTerminal("some-id");

    const [, args] = vi.mocked(invoke).mock.calls[0]!;
    expect(args).toHaveProperty("id", "some-id");
    expect(args).not.toHaveProperty("projectId");
  });

  it("passes a single options object as the second arg (not a positional string)", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openInTerminal("proj-123");

    const callArgs = vi.mocked(invoke).mock.calls[0]!;
    expect(typeof callArgs[1]).toBe("object");
    expect(callArgs[1]).not.toBeNull();
    expect(callArgs[2]).toBeUndefined();
  });

  it("resolved value is strictly undefined, not a Project or other object", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await openInTerminal("proj-123");

    expect(result).toBeUndefined();
    expect(typeof result).not.toBe("object");
  });
});

// ---------------------------------------------------------------------------
// T3.1 — listSequences / refreshSequences IPC wrappers
// ---------------------------------------------------------------------------

const MOCK_SEQUENCE: Sequence = {
  name: "my-sequence",
  description: "Does something useful.",
  path: "/home/user/project/.claude/sequences/my-sequence.md",
  mtime: "2026-05-20T12:00:00Z",
};

describe("listSequences IPC wrapper (T3.1)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof listSequences).toBe("function");
  });

  it("invokes 'list_sequences' with projectId under the 'projectId' key", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_SEQUENCE]);

    await listSequences("proj-abc");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("list_sequences", { projectId: "proj-abc" });
  });

  it("returns the Sequence array resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_SEQUENCE]);

    const result = await listSequences("proj-abc");

    expect(result).toHaveLength(1);
    expect(result[0]).toBe(MOCK_SEQUENCE);
  });

  it("returns an empty array when the project has no sequences", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    const result = await listSequences("proj-abc");

    expect(result).toEqual([]);
  });

  it("propagates NOT_FOUND rejection when the project does not exist", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-proj",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(listSequences("no-such-proj")).rejects.toEqual(error);
  });

  it("propagates IO rejection to the caller", async () => {
    const error = { code: "IO", message: "I/O error: permission denied", details: null };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(listSequences("proj-abc")).rejects.toEqual(error);
  });
});

describe("refreshSequences IPC wrapper (T3.1)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof refreshSequences).toBe("function");
  });

  it("invokes 'refresh_sequences' with projectId under the 'projectId' key", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_SEQUENCE]);

    await refreshSequences("proj-abc");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("refresh_sequences", { projectId: "proj-abc" });
  });

  it("returns the refreshed Sequence array resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_SEQUENCE]);

    const result = await refreshSequences("proj-abc");

    expect(result).toHaveLength(1);
    expect(result[0]).toBe(MOCK_SEQUENCE);
  });

  it("returns an empty array when the project has no sequences after refresh", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    const result = await refreshSequences("proj-abc");

    expect(result).toEqual([]);
  });

  it("propagates NOT_FOUND rejection when the project does not exist", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-proj",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(refreshSequences("no-such-proj")).rejects.toEqual(error);
  });

  it("propagates IO rejection to the caller", async () => {
    const error = { code: "IO", message: "I/O error: disk full", details: null };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(refreshSequences("proj-abc")).rejects.toEqual(error);
  });
});

// ---------------------------------------------------------------------------
// T3.1 gap tests — argument shape exactness
// Verify that listSequences / refreshSequences use the key 'projectId'
// (not 'id' as T2.8's openInEditor uses) and pass no extra keys.
// ---------------------------------------------------------------------------

describe("listSequences argument shape (T3.1 gap)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("passes the projectId under the key 'projectId', not 'id'", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    await listSequences("some-proj");

    const [, args] = vi.mocked(invoke).mock.calls[0]!;
    expect(args).toHaveProperty("projectId", "some-proj");
    expect(args).not.toHaveProperty("id");
  });

  it("passes a single options object as the second arg (not a positional string)", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    await listSequences("proj-123");

    const callArgs = vi.mocked(invoke).mock.calls[0]!;
    expect(typeof callArgs[1]).toBe("object");
    expect(callArgs[1]).not.toBeNull();
    expect(callArgs[2]).toBeUndefined();
  });

  it("returns an array (Promise<Sequence[]>), not undefined", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_SEQUENCE]);

    const result = await listSequences("proj-123");

    expect(Array.isArray(result)).toBe(true);
  });
});

describe("refreshSequences argument shape (T3.1 gap)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("passes the projectId under the key 'projectId', not 'id'", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    await refreshSequences("some-proj");

    const [, args] = vi.mocked(invoke).mock.calls[0]!;
    expect(args).toHaveProperty("projectId", "some-proj");
    expect(args).not.toHaveProperty("id");
  });

  it("passes a single options object as the second arg (not a positional string)", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    await refreshSequences("proj-123");

    const callArgs = vi.mocked(invoke).mock.calls[0]!;
    expect(typeof callArgs[1]).toBe("object");
    expect(callArgs[1]).not.toBeNull();
    expect(callArgs[2]).toBeUndefined();
  });

  it("returns an array (Promise<Sequence[]>), not undefined", async () => {
    vi.mocked(invoke).mockResolvedValue([MOCK_SEQUENCE]);

    const result = await refreshSequences("proj-123");

    expect(Array.isArray(result)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// T2.8 — TOAST_SHOW event constant (no events.test.ts exists; tested here)
// src/ipc/events.ts must export TOAST_SHOW matching the backend constant
// src-tauri/src/ipc/events.rs: pub const TOAST_SHOW: &str = "toast:show";
// ---------------------------------------------------------------------------

describe("TOAST_SHOW event constant (T2.8)", () => {
  it("TOAST_SHOW is exported from src/ipc/events.ts", async () => {
    const eventsModule = await import("./events");
    expect(eventsModule).toHaveProperty("TOAST_SHOW");
  });

  it("TOAST_SHOW equals the string 'toast:show' (matches backend constant)", async () => {
    const { TOAST_SHOW } = await import("./events");
    expect(TOAST_SHOW).toBe("toast:show");
  });

  it("CLI_LOST equals the string 'cli:lost' (existing constant regression guard)", async () => {
    const { CLI_LOST } = await import("./events");
    expect(CLI_LOST).toBe("cli:lost");
  });
});

// ---------------------------------------------------------------------------
// T4.3 — launchRun / stopRun / sendInput IPC wrappers
// ---------------------------------------------------------------------------

const MOCK_RUN: Run = {
  id: "01900000-0000-7000-8000-000000000001",
  project_id: "proj-abc",
  project_path: "/home/user/my-project",
  sequence_name: "build-and-test",
  attached_md_path: null,
  started_at: "2026-05-23T21:00:00Z",
  ended_at: null,
  status: "Pending" as RunStatus,
  exit_code: null,
  pid: null,
  note: null,
};

const MOCK_LAUNCH_INPUT: LaunchInput = {
  project_id: "proj-abc",
  sequence_name: "build-and-test",
  attached_md_path: null,
};

describe("launchRun IPC wrapper (T4.3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof launchRun).toBe("function");
  });

  it("invokes 'launch_run' with input under the 'input' key", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_RUN);

    await launchRun(MOCK_LAUNCH_INPUT);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("launch_run", { input: MOCK_LAUNCH_INPUT });
  });

  it("returns the Run resolved by invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(MOCK_RUN);

    const result = await launchRun(MOCK_LAUNCH_INPUT);

    expect(result).toBe(MOCK_RUN);
    expect(result.status).toBe("Pending");
  });

  it("propagates NOT_FOUND rejection when the project does not exist", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: project id: no-such-proj",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(launchRun({ ...MOCK_LAUNCH_INPUT, project_id: "no-such-proj" })).rejects.toEqual(
      error
    );
  });
});

describe("stopRun IPC wrapper (T4.3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof stopRun).toBe("function");
  });

  it("invokes 'stop_run' with runId under the 'runId' key", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await stopRun("01900000-0000-7000-8000-000000000001");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("stop_run", {
      runId: "01900000-0000-7000-8000-000000000001",
    });
  });

  it("returns Promise<void> (undefined) on success", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await stopRun("01900000-0000-7000-8000-000000000001");

    expect(result).toBeUndefined();
  });

  it("propagates NOT_FOUND rejection for unknown run id", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: run id: nonexistent-run",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(stopRun("nonexistent-run")).rejects.toEqual(error);
  });
});

describe("sendInput IPC wrapper (T4.3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof sendInput).toBe("function");
  });

  it("invokes 'send_input' with runId and text under the correct keys", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await sendInput("01900000-0000-7000-8000-000000000001", "yes");

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("send_input", {
      runId: "01900000-0000-7000-8000-000000000001",
      text: "yes",
    });
  });

  it("returns Promise<void> (undefined) on success", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    const result = await sendInput("01900000-0000-7000-8000-000000000001", "continue");

    expect(result).toBeUndefined();
  });

  it("propagates NOT_FOUND rejection for unknown run id", async () => {
    const error = {
      code: "NOT_FOUND",
      message: "Not found: run id: nonexistent-run",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(sendInput("nonexistent-run", "hello")).rejects.toEqual(error);
  });

  it("propagates INVALID_INPUT rejection when run is not accepting input", async () => {
    const error = {
      code: "INVALID_INPUT",
      message: "Invalid input: run is not accepting input",
      details: null,
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(sendInput("01900000-0000-7000-8000-000000000001", "test")).rejects.toEqual(error);
  });
});

describe("RUN_STARTED / RUN_EVENT / RUN_FINISHED event constants (T4.3)", () => {
  it("RUN_STARTED equals 'run:started'", async () => {
    const { RUN_STARTED } = await import("./events");
    expect(RUN_STARTED).toBe("run:started");
  });

  it("RUN_EVENT equals 'run:event'", async () => {
    const { RUN_EVENT } = await import("./events");
    expect(RUN_EVENT).toBe("run:event");
  });

  it("RUN_FINISHED equals 'run:finished'", async () => {
    const { RUN_FINISHED } = await import("./events");
    expect(RUN_FINISHED).toBe("run:finished");
  });
});
