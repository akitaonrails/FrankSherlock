import { useEffect, useRef } from "react";
import "./ContextMenu.css";

type Props = {
  x: number;
  y: number;
  selectedCount: number;
  onCopy: () => void;
  onRename: () => void;
  onDelete: () => void;
  onClose: () => void;
};

export default function ContextMenu({
  x, y, selectedCount, onCopy, onRename, onDelete, onClose,
}: Props) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Clamp position to viewport
    const el = menuRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    if (rect.right > window.innerWidth) {
      el.style.left = `${window.innerWidth - rect.width - 4}px`;
    }
    if (rect.bottom > window.innerHeight) {
      el.style.top = `${window.innerHeight - rect.height - 4}px`;
    }
  }, [x, y]);

  useEffect(() => {
    function handleClose(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    function handleScroll() { onClose(); }

    document.addEventListener("mousedown", handleClose);
    document.addEventListener("keydown", handleKey);
    window.addEventListener("scroll", handleScroll, true);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", handleClose);
      document.removeEventListener("keydown", handleKey);
      window.removeEventListener("scroll", handleScroll, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="context-menu"
      style={{ left: x, top: y }}
      role="menu"
    >
      <button className="context-menu-item" role="menuitem" onClick={onCopy}>
        <span>Copy</span>
        <span className="context-menu-shortcut">Ctrl+C</span>
      </button>

      {selectedCount === 1 && (
        <button className="context-menu-item" role="menuitem" onClick={onRename}>
          <span>Rename</span>
          <span className="context-menu-shortcut">F2</span>
        </button>
      )}

      <div className="context-menu-separator" role="separator" />

      <button className="context-menu-item danger" role="menuitem" onClick={onDelete}>
        <span>Delete</span>
        <span className="context-menu-shortcut">Del</span>
      </button>
    </div>
  );
}
