import { memo } from "react";
import type { ScanJobStatus } from "../../types";
import { basename } from "../../utils";

type ScanProgressProps = {
  scan: ScanJobStatus;
  readOnly: boolean;
  onCancel?: () => void;
  onResume?: () => void;
};

function formatEta(seconds: number): string {
  if (seconds < 60) return `~${Math.ceil(seconds)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.ceil(seconds % 60);
  if (mins < 60) return `~${mins}m ${secs}s`;
  const hrs = Math.floor(mins / 60);
  const remainMins = mins % 60;
  return `~${hrs}h ${remainMins}m`;
}

function computeEta(scan: ScanJobStatus): string | null {
  if (scan.processedFiles <= 0 || scan.totalFiles <= 0) return null;
  const remaining = scan.totalFiles - scan.processedFiles;
  if (remaining <= 0) return null;
  const now = Date.now() / 1000;
  const elapsed = now - scan.startedAt;
  if (elapsed <= 0) return null;
  const avgPerFile = elapsed / scan.processedFiles;
  const etaSeconds = avgPerFile * remaining;
  return formatEta(etaSeconds);
}

function ScanProgress({ scan, readOnly, onCancel, onResume }: ScanProgressProps) {
  if (scan.status === "interrupted") {
    return (
      <div className="sidebar-scan-progress">
        <div>Interrupted: {basename(scan.rootPath)}</div>
        {!readOnly && onResume && <button type="button" onClick={onResume}>Resume</button>}
      </div>
    );
  }

  const pct = scan.totalFiles
    ? Math.min(100, (scan.processedFiles / Math.max(1, scan.totalFiles)) * 100)
    : 0;
  const eta = computeEta(scan);

  return (
    <div className="sidebar-scan-progress">
      <div className="sidebar-scan-progress-header">
        {basename(scan.rootPath)}
        {eta && <span className="sidebar-scan-eta"> — {eta} remaining</span>}
      </div>
      <progress value={pct} max={100} />
      <div className="sidebar-scan-meta">
        +{scan.added} new, {scan.modified} mod, {scan.moved} moved
      </div>
      {!readOnly && onCancel && <button type="button" onClick={onCancel}>Cancel</button>}
    </div>
  );
}

export default memo(ScanProgress);
