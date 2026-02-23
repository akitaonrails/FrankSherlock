import type { Album } from "../../types";

type Props = {
  album: Album;
  isSelected: boolean;
  onSelect: () => void;
  onDelete: () => void;
};

export default function AlbumCard({ album, isSelected, onSelect, onDelete }: Props) {
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
            <rect x="1.5" y="2" width="13" height="12" rx="1.5" fill="#E8567C"/>
            <rect x="2.5" y="3" width="11" height="10" rx="1" fill="#F06292"/>
            <circle cx="8" cy="8" r="3" fill="rgba(255,255,255,0.25)"/>
            <circle cx="8" cy="8" r="1.2" fill="rgba(255,255,255,0.5)"/>
            <path d="M1.5 2h2v12h-2a1.5 1.5 0 010-3V5a1.5 1.5 0 010-3z" fill="#C62858" opacity="0.5"/>
          </svg>
        </span>
        <span className="root-card-name" title={album.name}>{album.name}</span>
        <button
          type="button"
          className="root-card-delete"
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
          title="Delete album"
          aria-label={`Delete ${album.name}`}
        >&times;</button>
      </div>
      <div className="root-card-meta">
        <span>{album.fileCount.toLocaleString()} files</span>
      </div>
    </div>
  );
}
