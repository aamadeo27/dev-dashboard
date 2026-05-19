import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Frontend IPC wrapper tests for log_frontend_error (T0.6)
//
// Tauri commands cannot be invoked in Vitest without a running Tauri window.
// These tests verify:
//   1. The wrapper function `logFrontendError` is exported from commands.ts.
//   2. It has the expected async function signature (message, stack?, route?).
//   3. It invokes the IPC command with the name "log_frontend_error".
//
// Actual Tauri invocation is mocked so no Tauri runtime is required.
// ---------------------------------------------------------------------------

// Mock @tauri-apps/api/core before importing the module under test so that
// `invoke` is replaced with a spy throughout the test suite.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

// Import after mock registration.
import { invoke } from "@tauri-apps/api/core";
import { logFrontendError } from "./commands";

describe("logFrontendError IPC wrapper", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("is exported as a function", () => {
    expect(typeof logFrontendError).toBe("function");
  });

  it("invokes the command named 'log_frontend_error'", () => {
    logFrontendError("test error");
    expect(invoke).toHaveBeenCalledWith(
      "log_frontend_error",
      expect.objectContaining({ message: "test error" })
    );
  });

  it("passes message, stack, and route when all are provided", () => {
    logFrontendError("TypeError: undefined", "at App.tsx:10", "/dashboard");
    expect(invoke).toHaveBeenCalledWith("log_frontend_error", {
      message: "TypeError: undefined",
      stack: "at App.tsx:10",
      route: "/dashboard",
    });
  });

  it("passes undefined for omitted optional args", () => {
    logFrontendError("bare message");
    expect(invoke).toHaveBeenCalledWith("log_frontend_error", {
      message: "bare message",
      stack: undefined,
      route: undefined,
    });
  });

  it("returns void (fire-and-forget — does not block caller)", () => {
    const result = logFrontendError("fire and forget");
    expect(result).toBeUndefined();
  });
});
