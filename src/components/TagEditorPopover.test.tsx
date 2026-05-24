// Unit tests for TagEditorPopover component. See docs/tasks/T2.7.md.
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Project } from "../ipc/bindings";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock("../ipc/commands", () => ({
  setProjectTags: vi.fn().mockResolvedValue({
    id: "proj-1",
    name: "my-app",
    path: "/home/user/my-app",
    tags: [],
    language: null,
    package_manager: null,
    added_at: "2026-01-01T00:00:00Z",
    last_modified: null,
    is_missing: false,
  }),
}));

import { setProjectTags } from "../ipc/commands";
import TagEditorPopover from "./TagEditorPopover";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: "proj-1",
    name: "my-app",
    path: "/home/user/my-app",
    tags: [],
    language: null,
    package_manager: null,
    added_at: "2026-01-01T00:00:00Z",
    last_modified: null,
    is_missing: false,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(setProjectTags).mockResolvedValue({
    id: "proj-1",
    name: "my-app",
    path: "/home/user/my-app",
    tags: [],
    language: null,
    package_manager: null,
    added_at: "2026-01-01T00:00:00Z",
    last_modified: null,
    is_missing: false,
  });
});

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

describe("TagEditorPopover — rendering", () => {
  it("renders the project name in the header", () => {
    render(
      <TagEditorPopover
        project={makeProject({ name: "cool-app" })}
        x={100}
        y={200}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    expect(screen.getByText("Tags for cool-app")).toBeTruthy();
  });

  it("renders existing tags as chips", () => {
    render(
      <TagEditorPopover
        project={makeProject({ tags: ["react", "typescript"] })}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    expect(screen.getByText("react")).toBeTruthy();
    expect(screen.getByText("typescript")).toBeTruthy();
  });

  it("shows 'No tags' when project has no tags", () => {
    render(
      <TagEditorPopover
        project={makeProject({ tags: [] })}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    expect(screen.getByText("No tags")).toBeTruthy();
  });

  it("renders an Add button", () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    expect(screen.getByRole("button", { name: "Add" })).toBeTruthy();
  });

  it("renders an input with placeholder 'Add tag...'", () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    expect(screen.getByPlaceholderText("Add tag...")).toBeTruthy();
  });

  it("positions the popover at given x/y as fixed", () => {
    const { container } = render(
      <TagEditorPopover
        project={makeProject()}
        x={50}
        y={80}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const el = container.firstChild as HTMLElement;
    expect(el.style.position).toBe("fixed");
    expect(el.style.left).toBe("50px");
    expect(el.style.top).toBe("80px");
  });

  it("has data-tag-editor attribute on container", () => {
    const { container } = render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const el = container.firstChild as HTMLElement;
    expect(el.hasAttribute("data-tag-editor")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Adding tags
// ---------------------------------------------------------------------------

describe("TagEditorPopover — adding tags", () => {
  it("adds a tag when Add button is clicked", async () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const input = screen.getByPlaceholderText("Add tag...");
    fireEvent.change(input, { target: { value: "newTag" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(setProjectTags).toHaveBeenCalledWith("proj-1", ["newtag"]);
    });
    expect(screen.getByText("newtag")).toBeTruthy();
  });

  it("adds a tag on Enter key", async () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const input = screen.getByPlaceholderText("Add tag...");
    fireEvent.change(input, { target: { value: "enterTag" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(setProjectTags).toHaveBeenCalledWith("proj-1", ["entertag"]);
    });
  });

  it("clears the input after adding a tag", async () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const input = screen.getByPlaceholderText("Add tag...");
    fireEvent.change(input, { target: { value: "sometag" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect((input as HTMLInputElement).value).toBe("");
    });
  });

  it("lowercases the tag before adding", async () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const input = screen.getByPlaceholderText("Add tag...");
    fireEvent.change(input, { target: { value: "MyTag" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(setProjectTags).toHaveBeenCalledWith("proj-1", ["mytag"]);
    });
  });

  it("does not add a duplicate tag", async () => {
    render(
      <TagEditorPopover
        project={makeProject({ tags: ["existing"] })}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    const input = screen.getByPlaceholderText("Add tag...");
    fireEvent.change(input, { target: { value: "existing" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      // setProjectTags should not be called since it's a duplicate
      expect(setProjectTags).not.toHaveBeenCalled();
    });
  });

  it("does not add an empty tag", async () => {
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(setProjectTags).not.toHaveBeenCalled();
    });
  });

  it("calls onTagsChange after adding a tag", async () => {
    const onTagsChange = vi.fn();
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={onTagsChange}
      />
    );
    const input = screen.getByPlaceholderText("Add tag...");
    fireEvent.change(input, { target: { value: "alpha" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(onTagsChange).toHaveBeenCalledWith("proj-1", ["alpha"]);
    });
  });
});

// ---------------------------------------------------------------------------
// Removing tags
// ---------------------------------------------------------------------------

describe("TagEditorPopover — removing tags", () => {
  it("removes a tag when the × button is clicked", async () => {
    render(
      <TagEditorPopover
        project={makeProject({ tags: ["react", "typescript"] })}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove tag react" }));

    await waitFor(() => {
      expect(setProjectTags).toHaveBeenCalledWith("proj-1", ["typescript"]);
    });
    expect(screen.queryByText("react")).toBeNull();
  });

  it("calls onTagsChange after removing a tag", async () => {
    const onTagsChange = vi.fn();
    render(
      <TagEditorPopover
        project={makeProject({ tags: ["beta"] })}
        x={0}
        y={0}
        onClose={vi.fn()}
        onTagsChange={onTagsChange}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove tag beta" }));

    await waitFor(() => {
      expect(onTagsChange).toHaveBeenCalledWith("proj-1", []);
    });
  });
});

// ---------------------------------------------------------------------------
// Escape / outside click closes
// ---------------------------------------------------------------------------

describe("TagEditorPopover — close behaviour", () => {
  it("calls onClose when Escape is pressed", () => {
    const onClose = vi.fn();
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={onClose}
        onTagsChange={vi.fn()}
      />
    );
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose on mousedown outside [data-tag-editor]", () => {
    const onClose = vi.fn();
    render(
      <TagEditorPopover
        project={makeProject()}
        x={0}
        y={0}
        onClose={onClose}
        onTagsChange={vi.fn()}
      />
    );
    fireEvent.mouseDown(document.body);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
