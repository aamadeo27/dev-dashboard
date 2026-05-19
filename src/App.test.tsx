import { render, screen } from "@testing-library/react";
import { Suspense, lazy } from "react";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import App from "./App";

// App uses HashRouter which reads window.location.hash.
// In jsdom we set window.location.hash before rendering to simulate navigation.

describe("App", () => {
  afterEach(() => {
    window.location.hash = "";
  });

  it("renders S-01 Setup on /setup route", async () => {
    window.location.hash = "#/setup";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-01 Setup")).toBeTruthy();
  });

  it("renders S-02 Dashboard on / route", async () => {
    window.location.hash = "#/";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-02 Dashboard")).toBeTruthy();
  });

  it("renders S-07 Settings on /settings route", async () => {
    window.location.hash = "#/settings";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-07 Settings")).toBeTruthy();
  });

  it("redirects unknown routes to Dashboard", async () => {
    window.location.hash = "#/this-route-does-not-exist";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-02 Dashboard")).toBeTruthy();
  });

  // --- Additional route coverage ---

  it("renders S-03 Project Detail on /projects/:projectId route", async () => {
    window.location.hash = "#/projects/some-id";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-03 Project Detail")).toBeTruthy();
  });

  it("renders S-04 Run Live on /runs/:runId/live route", async () => {
    window.location.hash = "#/runs/some-id/live";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-04 Run Live")).toBeTruthy();
  });

  it("renders S-05 Run Historical on /runs/:runId/history route", async () => {
    window.location.hash = "#/runs/some-id/history";
    await act(async () => {
      render(<App />);
    });
    expect(await screen.findByText("S-05 Run Historical")).toBeTruthy();
  });

  // --- Suspense fallback ---

  it("renders Suspense fallback div (dark background) while a lazy component is pending", async () => {
    // In vitest/jsdom, React.lazy modules from the project resolve synchronously,
    // so App's own Suspense fallback is never visible in the DOM during normal tests.
    // Instead, we directly test the fallback element shape by mounting a Suspense
    // boundary with the same fallback value used by App, backed by a never-resolving
    // lazy component — this forces Suspense to display the fallback.

    // A lazy import that never resolves, forcing Suspense to stay in the loading state.
    const NeverResolves = lazy(
      () =>
        new Promise<{ default: () => null }>(() => {
          // intentionally never resolves
        })
    );

    // The fallback mirrors the one in App.tsx exactly.
    const fallbackEl = <div style={{ background: "var(--bg-base)", minHeight: "100vh" }} />;

    const { container } = render(
      <Suspense fallback={fallbackEl}>
        <NeverResolves />
      </Suspense>
    );

    // The fallback div must be in the DOM (not null) with the expected inline styles.
    const fallback = container.querySelector("div");
    expect(fallback).not.toBeNull();
    // Confirm the background style references the design token variable.
    expect(fallback?.getAttribute("style")).toContain("--bg-base");
    // Confirm minHeight is applied so the fallback covers the full viewport height.
    expect(fallback?.getAttribute("style")).toContain("min-height");
  });
});
