import type { ScanJobStatus } from "../../types";
import { basename } from "../../utils";

type ScanProgressProps = {
  scan: ScanJobStatus;
  readOnly: boolean;
  onCancel?: () => void;
  onResume?: () => void;
};

export default function ScanProgress({ scan, readOnly, onCancel, onResume }: ScanProgressProps) {
  if (scan.status === "interrupted") {
    return (
      <div className="sidebar-scan-progress">
        <div>Interrupted: {basename(scan.rootPath)} at {scan.processedFiles} / {scan.totalFiles}</div>
        {!readOnly && onResume && <button type="button" onClick={onResume}>Resume</button>}
      </div>
    );
  }

  const pct = scan.totalFiles
    ? Math.min(100, (scan.processedFiles / Math.max(1, scan.totalFiles)) * 100)
    : 0;

  return (
    <div className="sidebar-scan-progress">
      <div className="sidebar-scan-progress-header">
        {basename(scan.rootPath)}: {scan.processedFiles} / {scan.totalFiles} ({pct.toFixed(1)}%)
      </div>
      <progress value={pct} max={100} />
      <div className="sidebar-scan-meta">
        +{scan.added} new, {scan.modified} mod, {scan.moved} moved
      </div>
      {!readOnly && onCancel && <button type="button" onClick={onCancel}>Cancel</button>}
    </div>
  );
}
