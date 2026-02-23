import { useEffect, useRef } from "react";
import type { Album } from "../../types";
import "./ContextMenu.css";

type Props = {
  x: number;
  y: number;
  selectedCount: number;
  albums: Album[];
  description: string | null;
  extractedText: string | null;
  onCopyPath: () => void;
  onCopyDescription: () => void;
  onCopyOcrText: () => void;
  onRename: () => void;
  onEditMetadata: () => void;
  onDelete: () => void;
  onAddToAlbum: (albumId: number) => void;
  onCreateAlbumFromSelection: () => void;
  onClose: () => void;
};

export default function ContextMenu({
  x, y, selectedCount, albums, description, extractedText,
  onCopyPath, onCopyDescription, onCopyOcrText, onRename, onEditMetadata, onDelete,
  onAddToAlbum, onCreateAlbumFromSelection, onClose,
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
      <button className="context-menu-item" role="menuitem" onClick={onCopyPath}>
        <span>Copy Path</span>
        <span className="context-menu-shortcut">Ctrl+C</span>
      </button>

      {selectedCount === 1 && description && (
        <button className="context-menu-item" role="menuitem" onClick={onCopyDescription}>
          <span>Copy Description</span>
        </button>
      )}

      {selectedCount === 1 && extractedText && (
        <button className="context-menu-item" role="menuitem" onClick={onCopyOcrText}>
          <span>Copy OCR Text</span>
        </button>
      )}

      {selectedCount === 1 && (
        <button className="context-menu-item" role="menuitem" onClick={onRename}>
          <span>Rename</span>
          <span className="context-menu-shortcut">F2</span>
        </button>
      )}

      {selectedCount === 1 && (
        <button className="context-menu-item" role="menuitem" onClick={onEditMetadata}>
          <span>Edit Metadata</span>
        </button>
      )}

      <div className="context-menu-parent" role="menuitem">
        <span>Add to Album</span>
        <span className="context-menu-arrow">&#9656;</span>
        <div className="context-menu-submenu" role="menu">
          {albums.map((album) => (
            <button
              key={album.id}
              className="context-menu-item"
              role="menuitem"
              onClick={() => onAddToAlbum(album.id)}
            >
              <span>{album.name}</span>
              <span className="context-menu-shortcut">{album.fileCount}</span>
            </button>
          ))}
          {albums.length > 0 && <div className="context-menu-separator" role="separator" />}
          <button
            className="context-menu-item"
            role="menuitem"
            onClick={onCreateAlbumFromSelection}
          >
            <span>New Album...</span>
          </button>
        </div>
      </div>

      <div className="context-menu-separator" role="separator" />

      <button className="context-menu-item danger" role="menuitem" onClick={onDelete}>
        <span>Delete</span>
        <span className="context-menu-shortcut">Del</span>
      </button>
    </div>
  );
}
