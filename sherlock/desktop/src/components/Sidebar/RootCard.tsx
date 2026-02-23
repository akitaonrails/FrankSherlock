import type { RootInfo, ScanJobStatus } from "../../types";

type RootCardProps = {
  root: RootInfo;
  isSelected: boolean;
  scan: ScanJobStatus | undefined;
  readOnly: boolean;
  onSelect: () => void;
  onDelete: () => void;
};

export default function RootCard({ root, isSelected, scan, readOnly, onSelect, onDelete }: RootCardProps) {
  const progress = scan?.totalFiles
    ? Math.min(100, (scan.processedFiles / Math.max(1, scan.totalFiles)) * 100)
    : 0;

  return (
    <div
      className={`root-card${isSelected ? " selected" : ""}`}
      onClick={onSelect}
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
      {scan && (
        <div className="root-card-scan">
          <progress value={progress} max={100} />
          <span>{scan.processedFiles}/{scan.totalFiles}</span>
        </div>
      )}
    </div>
  );
}
