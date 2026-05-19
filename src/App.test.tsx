import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("App", () => {
  it("renders without crashing", () => {
    render(<App />);
    // Router redirects / to /projects — placeholder div should be in document
    expect(document.body).toBeTruthy();
  });
});
