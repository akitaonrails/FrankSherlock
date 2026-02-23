import { getCurrentWindow } from "@tauri-apps/api/window";
import "./Titlebar.css";

const appWindow = getCurrentWindow();

type Props = {
  onClose: () => void;
  subtitle?: string | null;
  sidebarCollapsed?: boolean;
  onToggleSidebar?: () => void;
};

export default function Titlebar({ onClose, subtitle, sidebarCollapsed, onToggleSidebar }: Props) {
  return (
    <div className="titlebar" data-tauri-drag-region>
      {onToggleSidebar && (
        <button
          type="button"
          className="titlebar-toggle"
          onClick={onToggleSidebar}
          aria-label={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <rect x="1" y="2.5" width="12" height="1.5" rx="0.5" fill="currentColor" />
            <rect x="1" y="6.25" width="12" height="1.5" rx="0.5" fill="currentColor" />
            <rect x="1" y="10" width="12" height="1.5" rx="0.5" fill="currentColor" />
          </svg>
        </button>
      )}
      <span>Frank Sherlock{subtitle ? ` \u2014 ${subtitle}` : ""}</span>
      <div className="titlebar-controls">
        <button type="button" onClick={() => appWindow.minimize()} aria-label="Minimize">&#x2500;</button>
        <button type="button" onClick={() => appWindow.toggleMaximize()} aria-label="Maximize">&#x25A1;</button>
        <button type="button" className="close" onClick={onClose} aria-label="Close">&#x2715;</button>
      </div>
    </div>
  );
}
