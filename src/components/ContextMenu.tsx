// ContextMenu — see ui-ux-spec.md §6 and KB §4
import { useEffect } from "react";

export interface ContextMenuItem {
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}

interface ContextMenuProps {
  items: ContextMenuItem[];
  x: number;
  y: number;
  onClose: () => void;
}

export default function ContextMenu({ items, x, y, onClose }: ContextMenuProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onMouse = (e: MouseEvent) => {
      if (!(e.target as Element).closest("[data-context-menu]")) onClose();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onMouse);
    };
  }, [onClose]);

  return (
    <div
      data-context-menu
      style={{
        position: "fixed",
        top: y,
        left: x,
        zIndex: 1000,
        background: "var(--bg-elevated)",
        border: "1px solid var(--border-subtle)",
        borderRadius: "var(--radius-md)",
        padding: "var(--space-1) 0",
        minWidth: "180px",
        boxShadow: "0 4px 12px rgba(0,0,0,0.4)",
      }}
    >
      {items.map((item) => (
        <button
          key={item.label}
          type="button"
          disabled={item.disabled}
          onClick={() => {
            item.onClick();
            onClose();
          }}
          style={{
            display: "block",
            width: "100%",
            background: "transparent",
            border: "none",
            textAlign: "left",
            padding: "var(--space-2) var(--space-4)",
            fontSize: "var(--font-size-sm)",
            cursor: item.disabled ? "default" : "pointer",
            color: item.disabled
              ? "var(--text-disabled)"
              : item.danger
                ? "var(--error)"
                : "var(--text-primary)",
          }}
          onMouseEnter={(e) => {
            if (!item.disabled)
              (e.currentTarget as HTMLButtonElement).style.background = "var(--bg-hover)";
          }}
          onMouseLeave={(e) => {
            (e.currentTarget as HTMLButtonElement).style.background = "transparent";
          }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
