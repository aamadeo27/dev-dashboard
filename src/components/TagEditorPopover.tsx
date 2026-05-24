// TagEditorPopover — see ui-ux-spec.md §6 and KB §4
import { useEffect, useRef, useState } from "react";
import type { Project } from "../ipc/bindings";
import { setProjectTags } from "../ipc/commands";

interface TagEditorPopoverProps {
  project: Project;
  x: number;
  y: number;
  onClose: () => void;
  onTagsChange: (projectId: string, tags: string[]) => void;
}

export default function TagEditorPopover({
  project,
  x,
  y,
  onClose,
  onTagsChange,
}: TagEditorPopoverProps) {
  const [tags, setTags] = useState<string[]>(project.tags);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onMouse = (e: MouseEvent) => {
      if (!(e.target as Element).closest("[data-tag-editor]")) onClose();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onMouse);
    };
  }, [onClose]);

  const applyTags = (newTags: string[]) => {
    const previousTags = tags;
    setTags(newTags);
    setProjectTags(project.id, newTags)
      .then(() => {
        onTagsChange(project.id, newTags);
      })
      .catch(() => {
        setTags(previousTags);
      });
  };

  const handleAdd = () => {
    const trimmed = input.trim().toLowerCase();
    if (!trimmed || tags.includes(trimmed)) {
      setInput("");
      return;
    }
    applyTags([...tags, trimmed]);
    setInput("");
  };

  const handleRemove = (tag: string) => {
    applyTags(tags.filter((t) => t !== tag));
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAdd();
    }
  };

  return (
    <div
      data-tag-editor
      style={{
        position: "fixed",
        top: y,
        left: x,
        zIndex: 1001,
        background: "var(--bg-elevated)",
        border: "1px solid var(--border-subtle)",
        borderRadius: "var(--radius-lg)",
        padding: "var(--space-3)",
        minWidth: "220px",
        boxShadow: "0 4px 12px rgba(0,0,0,0.4)",
      }}
    >
      <div
        style={{
          fontSize: "var(--font-size-sm)",
          color: "var(--text-secondary)",
          marginBottom: "var(--space-2)",
        }}
      >
        Tags for {project.name}
      </div>
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: "var(--space-1)",
          marginBottom: "var(--space-2)",
        }}
      >
        {tags.map((tag) => (
          <span
            key={tag}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "4px",
              background: "var(--primary-dim)",
              color: "var(--primary)",
              borderRadius: "var(--radius-sm)",
              padding: "2px 6px",
              fontSize: "var(--font-size-xs)",
            }}
          >
            {tag}
            <button
              type="button"
              aria-label={`Remove tag ${tag}`}
              onClick={() => handleRemove(tag)}
              style={{
                background: "transparent",
                border: "none",
                cursor: "pointer",
                color: "var(--primary)",
                padding: 0,
                lineHeight: 1,
                fontSize: "12px",
              }}
            >
              ×
            </button>
          </span>
        ))}
        {tags.length === 0 && (
          <span style={{ fontSize: "var(--font-size-xs)", color: "var(--text-disabled)" }}>
            No tags
          </span>
        )}
      </div>
      <div style={{ display: "flex", gap: "var(--space-1)" }}>
        <input
          ref={inputRef}
          type="text"
          placeholder="Add tag..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          style={{
            flex: 1,
            background: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-sm)",
            padding: "var(--space-1) var(--space-2)",
            fontSize: "var(--font-size-xs)",
            color: "var(--text-primary)",
            outline: "none",
          }}
        />
        <button
          type="button"
          onClick={handleAdd}
          style={{
            background: "var(--primary)",
            border: "none",
            borderRadius: "var(--radius-sm)",
            padding: "var(--space-1) var(--space-2)",
            fontSize: "var(--font-size-xs)",
            color: "var(--text-on-primary)",
            cursor: "pointer",
          }}
        >
          Add
        </button>
      </div>
    </div>
  );
}
