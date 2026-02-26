import { convertFileSrc } from "@tauri-apps/api/core";
import { fileName } from "../../utils/format";
import type { SearchItem } from "../../types";

type Props = {
  item: SearchItem;
  index: number;
  isSelected: boolean;
  isFocused: boolean;
  onClick: (index: number, e: React.MouseEvent) => void;
  onDoubleClick: (index: number) => void;
  onContextMenu: (index: number, e: React.MouseEvent) => void;
};

export default function ImageTile({ item, index, isSelected, isFocused, onClick, onDoubleClick, onContextMenu }: Props) {
  const thumb = item.thumbnailPath ? convertFileSrc(item.thumbnailPath) : null;
  const isVideo = item.mediaType === "video";

  return (
    <article
      className={`tile${isSelected ? " tile-selected" : ""}${isFocused ? " tile-focused" : ""}`}
      role="listitem"
      onClick={(e) => onClick(index, e)}
      onDoubleClick={() => onDoubleClick(index)}
      onContextMenu={(e) => onContextMenu(index, e)}
    >
      <div className="tile-thumb">
        {thumb ? (
          <img src={thumb} alt={item.relPath} loading="lazy" />
        ) : (
          <div className="tile-thumb-placeholder">
            <span className="badge">{item.mediaType}</span>
          </div>
        )}
        {isVideo && (
          <span className="tile-video-badge" aria-label="Video">&#9654;</span>
        )}
        {item.faceCount != null && item.faceCount > 0 && (
          <span className="tile-face-badge" aria-label={`${item.faceCount} face${item.faceCount > 1 ? "s" : ""}`}>
            {item.faceCount}
          </span>
        )}
      </div>
      <div className="tile-filename">
        <span>{fileName(item.relPath)}</span>
      </div>
      <div className="tile-hover-overlay">
        <h3>{fileName(item.relPath)}</h3>
        <p>{item.description || "No description yet"}</p>
        <div className="tile-meta">
          <span className="badge">{item.mediaType}</span>
          <span>{item.confidence.toFixed(2)}</span>
        </div>
      </div>
    </article>
  );
}
