import { convertFileSrc } from "@tauri-apps/api/core";
import type { DuplicatesResponse, DuplicateFile, DuplicateGroup } from "../../types";
import { fileName } from "../../utils/format";
import { formatBytes } from "../../utils/format";
import "./DuplicatesView.css";

type Props = {
  data: DuplicatesResponse;
  loading: boolean;
  selected: Set<number>;
  nearEnabled: boolean;
  nearThreshold: number;
  onNearEnabledChange: (enabled: boolean) => void;
  onNearThresholdChange: (value: number) => void;
  onToggleFile: (fileId: number) => void;
  onSelectAllDuplicates: () => void;
  onDeselectAll: () => void;
  onDeleteSelected: () => void;
  onBack: () => void;
  onSelectGroupDuplicates: (group: DuplicateGroup) => void;
  onPreviewFile: (file: DuplicateFile) => void;
};

function formatDate(mtimeNs: number): string {
  const ms = mtimeNs / 1_000_000;
  return new Date(ms).toLocaleDateString();
}

export default function DuplicatesView({
  data, loading, selected,
  nearEnabled, nearThreshold, onNearEnabledChange, onNearThresholdChange,
  onToggleFile, onSelectAllDuplicates, onDeselectAll, onDeleteSelected,
  onBack, onSelectGroupDuplicates, onPreviewFile,
}: Props) {
  return (
    <div className="duplicates-view">
      <div className="duplicates-toolbar">
        <div className="duplicates-stats">
          <strong>{data.totalGroups}</strong> group{data.totalGroups !== 1 ? "s" : ""},
          {" "}<strong>{data.totalDuplicateFiles}</strong> duplicate{data.totalDuplicateFiles !== 1 ? "s" : ""},
          {" "}<strong>{formatBytes(data.totalWastedBytes)}</strong> wasted
        </div>
        <label className="near-toggle" title="Include visually similar files (not just exact copies)">
          <input
            type="checkbox"
            checked={nearEnabled}
            onChange={(e) => onNearEnabledChange(e.target.checked)}
          />
          Near-duplicates
        </label>
        {nearEnabled && (
          <div className="near-threshold-control">
            <input
              type="range"
              min={70}
              max={99}
              value={Math.round(nearThreshold * 100)}
              onChange={(e) => onNearThresholdChange(Number(e.target.value) / 100)}
            />
            <span className="near-threshold-label">{Math.round(nearThreshold * 100)}%</span>
          </div>
        )}
        {selected.size > 0 ? (
          <button type="button" onClick={onDeselectAll}>Deselect all</button>
        ) : (
          <button type="button" onClick={onSelectAllDuplicates} disabled={data.totalDuplicateFiles === 0}>
            Select all duplicates
          </button>
        )}
        <button
          type="button"
          className="danger-btn"
          disabled={selected.size === 0}
          onClick={onDeleteSelected}
        >
          Delete selected ({selected.size})
        </button>
        <button type="button" onClick={onBack}>Back</button>
      </div>

      <div className="duplicates-body">
        {loading && <div className="duplicates-loading">Searching for duplicates...</div>}
        {!loading && data.totalGroups === 0 && (
          <div className="duplicates-empty">No duplicate files found.</div>
        )}
        {data.groups.map((group) => (
          <GroupCard
            key={group.fingerprint}
            group={group}
            selected={selected}
            onToggleFile={onToggleFile}
            onSelectGroupDuplicates={onSelectGroupDuplicates}
            onPreviewFile={onPreviewFile}
          />
        ))}
      </div>
    </div>
  );
}

function GroupCard({
  group, selected, onToggleFile, onSelectGroupDuplicates, onPreviewFile,
}: {
  group: DuplicateGroup;
  selected: Set<number>;
  onToggleFile: (fileId: number) => void;
  onSelectGroupDuplicates: (group: DuplicateGroup) => void;
  onPreviewFile: (file: DuplicateFile) => void;
}) {
  return (
    <div className="dup-group" data-group-type={group.groupType}>
      <div className="dup-group-header">
        <span className={`dup-type-badge dup-type-badge-${group.groupType}`}>
          {group.groupType === "exact" ? "EXACT" : "NEAR"}
        </span>
        <div className="dup-group-info">
          <strong>{group.fileCount}</strong> copies &middot; {formatBytes(group.wastedBytes)} wasted
          {group.groupType === "near" && group.avgSimilarity != null && (
            <span className="dup-similarity-label">
              {" "}&middot; {Math.round(group.avgSimilarity * 100)}% similar
            </span>
          )}
        </div>
        <button type="button" onClick={() => onSelectGroupDuplicates(group)}>
          Select duplicates
        </button>
      </div>
      {group.files.map((file) => (
        <FileRow
          key={file.id}
          file={file}
          isSelected={selected.has(file.id)}
          onToggle={() => onToggleFile(file.id)}
          onPreview={() => onPreviewFile(file)}
        />
      ))}
    </div>
  );
}

function FileRow({
  file, isSelected, onToggle, onPreview,
}: {
  file: DuplicateFile;
  isSelected: boolean;
  onToggle: () => void;
  onPreview: () => void;
}) {
  const thumb = file.thumbnailPath ? convertFileSrc(file.thumbnailPath) : null;

  return (
    <div
      className={`dup-file-row${isSelected ? " dup-file-row-selected" : ""}`}
      onClick={onPreview}
    >
      <input
        type="checkbox"
        className="dup-file-checkbox"
        checked={isSelected}
        onChange={onToggle}
        onClick={(e) => e.stopPropagation()}
        aria-label={`Select ${fileName(file.relPath)}`}
      />
      <div className="dup-file-thumb">
        {thumb ? (
          <img src={thumb} alt={fileName(file.relPath)} loading="lazy" />
        ) : (
          <span className="dup-file-thumb-placeholder">{file.mediaType}</span>
        )}
      </div>
      <div className="dup-file-info">
        <div className="dup-file-path" title={file.absPath}>{file.relPath}</div>
        <div className="dup-file-meta">
          <span>{file.rootPath}</span>
          <span>{formatBytes(file.sizeBytes)}</span>
          <span>{formatDate(file.mtimeNs)}</span>
          {file.groupType === "near" && file.similarityScore != null && !file.isKeeper && (
            <span className="dup-file-similarity">{Math.round(file.similarityScore * 100)}%</span>
          )}
        </div>
      </div>
      {file.isKeeper && <span className="dup-keeper-badge">KEEP</span>}
    </div>
  );
}
