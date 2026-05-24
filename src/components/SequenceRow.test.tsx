// Unit tests for SequenceRow component. See docs/tasks/T3.2.md.
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Sequence } from "../ipc/bindings";
import { SequenceRow } from "./SequenceRow";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeSequence(overrides: Partial<Sequence> = {}): Sequence {
  return {
    name: "build-and-test",
    description: "Runs build, lint, and unit tests for the project.",
    path: "/home/user/.config/dev-dashboard/sequences/build-and-test.md",
    mtime: "2026-05-01T10:00:00Z",
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Reset mocks
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("SequenceRow — rendering", () => {
  it("1. renders sequence name", () => {
    const seq = makeSequence({ name: "my-sequence" });
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    expect(screen.getByText("my-sequence")).toBeTruthy();
  });

  it("2. renders sequence description", () => {
    const seq = makeSequence({ description: "Deploys the app to staging." });
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    expect(screen.getByText("Deploys the app to staging.")).toBeTruthy();
  });

  it("3. Run button has aria-label 'Run <name>'", () => {
    const seq = makeSequence({ name: "deploy-staging" });
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Run deploy-staging" })).toBeTruthy();
  });
});

describe("SequenceRow — interactions", () => {
  it("4. onRun called with correct sequence when Run button is clicked", () => {
    const onRun = vi.fn();
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={onRun} />);
    fireEvent.click(screen.getByRole("button", { name: `Run ${seq.name}` }));
    expect(onRun).toHaveBeenCalledTimes(1);
    expect(onRun).toHaveBeenCalledWith(seq);
  });

  it("5. onSelect called with correct sequence when row is clicked", () => {
    const onSelect = vi.fn();
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} onSelect={onSelect} />);
    // Click the listitem (the row container)
    const listitem = screen.getByRole("listitem");
    fireEvent.click(listitem);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(seq);
  });

  it("6. Run button click does NOT trigger onSelect (stopPropagation)", () => {
    const onSelect = vi.fn();
    const onRun = vi.fn();
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={onRun} onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("button", { name: `Run ${seq.name}` }));
    expect(onRun).toHaveBeenCalledTimes(1);
    expect(onSelect).not.toHaveBeenCalled();
  });
});

describe("SequenceRow — selected state", () => {
  it("7. selected=true gives row data-selected='true' attribute", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} selected={true} onRun={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    expect(listitem.getAttribute("data-selected")).toBe("true");
  });

  it("7b. selected=true gives row aria-selected=true", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} selected={true} onRun={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    expect(listitem.getAttribute("aria-selected")).toBe("true");
  });

  it("7c. selected=false (default) gives row data-selected='false' attribute", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    expect(listitem.getAttribute("data-selected")).toBe("false");
  });
});

describe("SequenceRow — keyboard navigation", () => {
  it("8. pressing Enter on row triggers onSelect", () => {
    const onSelect = vi.fn();
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} onSelect={onSelect} />);
    const listitem = screen.getByRole("listitem");
    fireEvent.keyDown(listitem, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(seq);
  });

  it("8b. pressing Space on row triggers onSelect", () => {
    const onSelect = vi.fn();
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} onSelect={onSelect} />);
    const listitem = screen.getByRole("listitem");
    fireEvent.keyDown(listitem, { key: " " });
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(seq);
  });

  it("8c. pressing other key on row does NOT trigger onSelect", () => {
    const onSelect = vi.fn();
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} onSelect={onSelect} />);
    const listitem = screen.getByRole("listitem");
    fireEvent.keyDown(listitem, { key: "Escape" });
    expect(onSelect).not.toHaveBeenCalled();
  });
});

describe("SequenceRow — long description wrapping", () => {
  it("description element does not have overflow:hidden or text-overflow:ellipsis", () => {
    const seq = makeSequence({
      description:
        "This is a very long description that should wrap cleanly within the card width without any truncation or ellipsis applied to it whatsoever.",
    });
    const { container } = render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    // Find the element containing the description text
    const descEl = container.querySelector('[style*="break-word"]') as HTMLElement | null;
    expect(descEl).not.toBeNull();
    expect(descEl!.style.overflow).not.toBe("hidden");
    expect(descEl!.style.textOverflow).not.toBe("ellipsis");
  });

  it("description element has wordBreak: break-word style", () => {
    const seq = makeSequence({ description: "A description." });
    const { container } = render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    const descEl = container.querySelector('[style*="break-word"]') as HTMLElement | null;
    expect(descEl).not.toBeNull();
    expect(descEl!.style.wordBreak).toBe("break-word");
  });
});

describe("SequenceRow — aria-selected false case", () => {
  it("7d. selected=false (default) gives row aria-selected='false'", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    expect(listitem.getAttribute("aria-selected")).toBe("false");
  });

  it("7e. selected=false explicitly gives row aria-selected='false'", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} selected={false} onRun={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    expect(listitem.getAttribute("aria-selected")).toBe("false");
  });
});

describe("SequenceRow — tabIndex behaviour", () => {
  it("row has tabIndex=0 when onSelect is provided", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} onSelect={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    expect(listitem.getAttribute("tabindex")).toBe("0");
  });

  it("row has no tabIndex when onSelect is not provided", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    const listitem = screen.getByRole("listitem");
    // tabIndex should be absent (undefined → not rendered) so not keyboard-reachable
    expect(listitem.getAttribute("tabindex")).toBeNull();
  });
});

describe("SequenceRow — role", () => {
  it("row container has role='listitem'", () => {
    const seq = makeSequence();
    render(<SequenceRow sequence={seq} onRun={vi.fn()} />);
    expect(screen.getByRole("listitem")).toBeTruthy();
  });
});
