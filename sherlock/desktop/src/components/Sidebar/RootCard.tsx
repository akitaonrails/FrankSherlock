import { useState, useEffect, useRef, useCallback } from "react";
import type { RootInfo, ScanJobStatus } from "../../types";
import { computeEta } from "../../utils/scanEta";

type RootCardProps = {
  root: RootInfo;
  isSelected: boolean;
  scan: ScanJobStatus | undefined;
  readOnly: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onRescan: () => void;
  onRefresh: () => void;
  onCopyPath: () => void;
  onCancelScan?: () => void;
  onResumeScan?: () => void;
};

export default function RootCard({ root, isSelected, scan, readOnly, onSelect, onDelete, onRescan, onRefresh, onCopyPath, onCancelScan, onResumeScan }: RootCardProps) {
  const [showMenu, setShowMenu] = useState(false);
  const [menuPos, setMenuPos] = useState({ x: 0, y: 0 });
  const menuRef = useRef<HTMLDivElement>(null);

  const closeMenu = useCallback(() => setShowMenu(false), []);

  const progress = scan?.totalFiles
    ? Math.min(100, (scan.processedFiles / Math.max(1, scan.totalFiles)) * 100)
    : 0;

  function handleContextMenu(e: React.MouseEvent) {
    if (readOnly) return;
    e.preventDefault();
    e.stopPropagation();
    setMenuPos({ x: e.clientX, y: e.clientY });
    setShowMenu(true);
  }

  // Viewport clamping
  useEffect(() => {
    if (!showMenu) return;
    const el = menuRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    if (rect.right > window.innerWidth) {
      el.style.left = `${window.innerWidth - rect.width - 4}px`;
    }
    if (rect.bottom > window.innerHeight) {
      el.style.top = `${window.innerHeight - rect.height - 4}px`;
    }
  }, [showMenu, menuPos]);

  // Click-away and Escape dismiss
  useEffect(() => {
    if (!showMenu) return;
    function handleMouseDown(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        closeMenu();
      }
    }
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") closeMenu();
    }
    document.addEventListener("mousedown", handleMouseDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleMouseDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [showMenu, closeMenu]);

  const eta = scan?.status === "running" ? computeEta(scan) : null;

  return (
    <div
      className={`root-card${isSelected ? " selected" : ""}`}
      onClick={onSelect}
      onContextMenu={handleContextMenu}
      title={root.rootPath}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
    >
      <div className="root-card-header">
        <span className="root-card-icon">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M1.5 3C1.5 2.17 2.17 1.5 3 1.5h3.59a1.5 1.5 0 011.06.44L8.71 3H13a1.5 1.5 0 011.5 1.5v8A1.5 1.5 0 0113 14H3a1.5 1.5 0 01-1.5-1.5V3z" fill="#5AC8FA"/>
            <path d="M1.5 5.5h13v7a1.5 1.5 0 01-1.5 1.5H3a1.5 1.5 0 01-1.5-1.5v-7z" fill="#34AADC"/>
            <path d="M1.5 5.5h13v1H1.5z" fill="rgba(0,0,0,0.08)"/>
          </svg>
        </span>
        <span className="root-card-name" title={root.rootPath}>{root.rootName}</span>
        {!readOnly && (
          <button
            type="button"
            className="root-card-delete"
            onClick={(e) => { e.stopPropagation(); onDelete(); }}
            title="Remove folder"
            aria-label={`Remove ${root.rootName}`}
          >&times;</button>
        )}
      </div>
      <div className="root-card-meta">
        <span>{root.fileCount.toLocaleString()} files</span>
      </div>
      {scan?.status === "running" && scan.phase === "discovering" && (
        <div className="root-card-scan">
          <div className="root-card-discovery-bar" />
          <span>Discovering files... ({scan.discoveredFiles.toLocaleString()} found)</span>
          {!readOnly && onCancelScan && (
            <button type="button" className="root-card-scan-btn" onClick={(e) => { e.stopPropagation(); onCancelScan(); }}>Pause</button>
          )}
        </div>
      )}
      {scan?.status === "running" && scan.phase === "thumbnailing" && (
        <div className="root-card-scan">
          <progress value={progress} max={100} />
          <span>Thumbnailing {scan.processedFiles}/{scan.totalFiles}</span>
          <div className="root-card-scan-stats">
            +{scan.added} new, {scan.modified} mod, {scan.moved} moved
          </div>
          {!readOnly && onCancelScan && (
            <button type="button" className="root-card-scan-btn" onClick={(e) => { e.stopPropagation(); onCancelScan(); }}>Pause</button>
          )}
        </div>
      )}
      {scan?.status === "running" && (scan.phase === "classifying" || scan.phase === "processing") && (
        <div className="root-card-scan">
          <progress value={progress} max={100} />
          <span>Classifying {scan.processedFiles}/{scan.totalFiles}</span>
          {eta && <span className="root-card-eta">{eta} remaining</span>}
          {!readOnly && onCancelScan && (
            <button type="button" className="root-card-scan-btn" onClick={(e) => { e.stopPropagation(); onCancelScan(); }}>Pause</button>
          )}
        </div>
      )}
      {scan?.status === "interrupted" && (
        <div className="root-card-scan root-card-scan-interrupted">
          <span>Scan interrupted</span>
          {!readOnly && onResumeScan && (
            <button type="button" className="root-card-scan-btn" onClick={(e) => { e.stopPropagation(); onResumeScan(); }}>Resume</button>
          )}
        </div>
      )}
      {showMenu && (
        <div
          ref={menuRef}
          className="root-card-context-menu"
          style={{ left: menuPos.x, top: menuPos.y }}
          role="menu"
        >
          <button role="menuitem" onClick={(e) => { e.stopPropagation(); onCopyPath(); setShowMenu(false); }}>Copy Path</button>
          <button role="menuitem" onClick={(e) => { e.stopPropagation(); onRefresh(); setShowMenu(false); }}>Refresh Metadata</button>
          <button role="menuitem" onClick={(e) => { e.stopPropagation(); onRescan(); setShowMenu(false); }}>Rescan</button>
          <button role="menuitem" className="danger" onClick={(e) => { e.stopPropagation(); onDelete(); setShowMenu(false); }}>Remove</button>
        </div>
      )}
    </div>
  );
}
