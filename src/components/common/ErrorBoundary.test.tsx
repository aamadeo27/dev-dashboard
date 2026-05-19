import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    render(
      <ErrorBoundary>
        <div data-testid="child">ok</div>
      </ErrorBoundary>
    );
    expect(screen.getByTestId("child")).toBeTruthy();
  });
});
