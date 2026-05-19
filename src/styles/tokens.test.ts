import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// File-content smoke test for tokens.css.
// Purpose: catch accidental deletion of design tokens in future refactors.
// This is NOT a runtime CSS test — it reads the file as text and asserts
// that the expected CSS custom property names are present.

const tokensPath = resolve(__dirname, "tokens.css");
const tokensContent = readFileSync(tokensPath, "utf-8");

describe("tokens.css — design token presence", () => {
  // Helper: assert a CSS variable declaration is present in the file.
  function hasToken(name: string): boolean {
    // Match "--token-name:" requiring it to be preceded by whitespace or start-of-line
    // (prevents partial-name matches like "--bg-base" matching "--bg-base-foo").
    return new RegExp(`(^|\\s)${name}\\s*:`, "m").test(tokensContent);
  }

  // --- Color tokens (UI spec §1.1) ---
  it("contains --bg-base", () => {
    expect(hasToken("--bg-base")).toBe(true);
  });

  it("contains --bg-surface", () => {
    expect(hasToken("--bg-surface")).toBe(true);
  });

  it("contains --bg-elevated", () => {
    expect(hasToken("--bg-elevated")).toBe(true);
  });

  it("contains --text-primary", () => {
    expect(hasToken("--text-primary")).toBe(true);
  });

  it("contains --text-secondary", () => {
    expect(hasToken("--text-secondary")).toBe(true);
  });

  it("contains --accent-primary (mapped as --accent)", () => {
    // The spec names this role "accent-primary"; the token is --accent.
    expect(hasToken("--accent")).toBe(true);
  });

  it("contains --primary (violet accent)", () => {
    expect(hasToken("--primary")).toBe(true);
  });

  it("contains --error", () => {
    expect(hasToken("--error")).toBe(true);
  });

  it("contains --success", () => {
    expect(hasToken("--success")).toBe(true);
  });

  // --- Typography tokens (UI spec §1.2) ---
  it("contains --font-size-base", () => {
    expect(hasToken("--font-size-base")).toBe(true);
  });

  it("contains --font-size-sm", () => {
    expect(hasToken("--font-size-sm")).toBe(true);
  });

  it("contains --font-size-xs", () => {
    expect(hasToken("--font-size-xs")).toBe(true);
  });

  it("contains --font-size-code", () => {
    expect(hasToken("--font-size-code")).toBe(true);
  });

  it("contains --font-size-lg", () => {
    expect(hasToken("--font-size-lg")).toBe(true);
  });

  // --- Spacing tokens (UI spec §1.3) ---
  it("contains --space-1 (4px)", () => {
    expect(hasToken("--space-1")).toBe(true);
  });

  it("contains --space-4 (16px)", () => {
    expect(hasToken("--space-4")).toBe(true);
  });

  it("contains --space-6 (24px)", () => {
    expect(hasToken("--space-6")).toBe(true);
  });

  it("contains --space-8 (32px)", () => {
    expect(hasToken("--space-8")).toBe(true);
  });

  // --- Motion tokens (UI spec §1.6) ---
  it("contains --duration-fast", () => {
    expect(hasToken("--duration-fast")).toBe(true);
  });

  it("contains --duration-base", () => {
    expect(hasToken("--duration-base")).toBe(true);
  });

  it("contains --duration-slow", () => {
    expect(hasToken("--duration-slow")).toBe(true);
  });

  // --- Token value spot-checks (catch value regressions, not just name presence) ---
  it("--bg-base has value #0f1117", () => {
    expect(tokensContent).toContain("--bg-base: #0f1117");
  });

  it("--font-size-base has value 14px", () => {
    expect(tokensContent).toContain("--font-size-base: 14px");
  });

  it("--font-size-code has value 12px", () => {
    expect(tokensContent).toContain("--font-size-code: 12px");
  });

  it("--space-4 has value 16px", () => {
    expect(tokensContent).toContain("--space-4: 16px");
  });

  it("--duration-fast has value 150ms", () => {
    expect(tokensContent).toContain("--duration-fast: 150ms");
  });
});
