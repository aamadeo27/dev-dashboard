import { describe, expect, it } from "vitest";
import { isDev } from "./env";

describe("isDev", () => {
  it("is a boolean", () => {
    expect(typeof isDev).toBe("boolean");
  });
});
