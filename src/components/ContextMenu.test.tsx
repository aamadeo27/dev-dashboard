// Unit tests for ContextMenu component. See docs/tasks/T2.7.md.
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ContextMenu from "./ContextMenu";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeItems(
  overrides: Array<
    Partial<{ label: string; onClick: () => void; danger?: boolean; disabled?: boolean }>
  > = []
) {
  return overrides.map((o) => ({
    label: o.label ?? "Item",
    onClick: o.onClick ?? vi.fn(),
    danger: o.danger,
    disabled: o.disabled,
  }));
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

describe("ContextMenu — rendering", () => {
  it("renders each item as a button", () => {
    render(
      <ContextMenu
        items={makeItems([{ label: "Open in Editor" }, { label: "Remove" }])}
        x={100}
        y={200}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByRole("button", { name: "Open in Editor" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy();
  });

  it("positions the menu at the given x/y coordinates (fixed)", () => {
    const { container } = render(
      <ContextMenu items={makeItems([{ label: "Item" }])} x={42} y={99} onClose={vi.fn()} />
    );
    const menu = container.firstChild as HTMLElement;
    expect(menu.style.position).toBe("fixed");
    expect(menu.style.left).toBe("42px");
    expect(menu.style.top).toBe("99px");
  });

  it("applies data-context-menu attribute to the container", () => {
    const { container } = render(
      <ContextMenu items={makeItems([{ label: "A" }])} x={0} y={0} onClose={vi.fn()} />
    );
    const menu = container.firstChild as HTMLElement;
    expect(menu.hasAttribute("data-context-menu")).toBe(true);
  });

  it("renders a disabled button when item.disabled is true", () => {
    render(
      <ContextMenu
        items={makeItems([{ label: "Disabled Item", disabled: true }])}
        x={0}
        y={0}
        onClose={vi.fn()}
      />
    );
    const btn = screen.getByRole("button", { name: "Disabled Item" });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Click behaviour
// ---------------------------------------------------------------------------

describe("ContextMenu — click behaviour", () => {
  it("calls item.onClick when button is clicked", () => {
    const onClick = vi.fn();
    render(
      <ContextMenu
        items={makeItems([{ label: "Click Me", onClick }])}
        x={0}
        y={0}
        onClose={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "Click Me" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("calls onClose after item.onClick", () => {
    const onClose = vi.fn();
    render(<ContextMenu items={makeItems([{ label: "A" }])} x={0} y={0} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "A" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not call onClick for a disabled item when clicked", () => {
    const onClick = vi.fn();
    render(
      <ContextMenu
        items={makeItems([{ label: "Disabled", onClick, disabled: true }])}
        x={0}
        y={0}
        onClose={vi.fn()}
      />
    );
    // Clicking a disabled button does not fire events in JSDOM
    const btn = screen.getByRole("button", { name: "Disabled" });
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Keyboard: Escape closes the menu
// ---------------------------------------------------------------------------

describe("ContextMenu — Escape key", () => {
  it("calls onClose when Escape is pressed", () => {
    const onClose = vi.fn();
    render(<ContextMenu items={makeItems([{ label: "A" }])} x={0} y={0} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not call onClose when a non-Escape key is pressed", () => {
    const onClose = vi.fn();
    render(<ContextMenu items={makeItems([{ label: "A" }])} x={0} y={0} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onClose).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Outside click closes the menu
// ---------------------------------------------------------------------------

describe("ContextMenu — outside click", () => {
  it("calls onClose when mousedown fires outside [data-context-menu]", () => {
    const onClose = vi.fn();
    render(<ContextMenu items={makeItems([{ label: "A" }])} x={0} y={0} onClose={onClose} />);
    // Simulate a click on document body (outside the menu)
    fireEvent.mouseDown(document.body);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// Danger styling: color differs for danger items
// ---------------------------------------------------------------------------

describe("ContextMenu — danger item", () => {
  it("renders a danger item without crashing", () => {
    render(
      <ContextMenu
        items={makeItems([{ label: "Remove", danger: true }])}
        x={0}
        y={0}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy();
  });
});
