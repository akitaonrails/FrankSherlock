import type { SmartFolder } from "../../types";

type Props = {
  folder: SmartFolder;
  isSelected: boolean;
  onSelect: () => void;
  onDelete: () => void;
};

export default function SmartFolderCard({ folder, isSelected, onSelect, onDelete }: Props) {
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
            <path d="M1.5 3C1.5 2.17 2.17 1.5 3 1.5h3.59a1.5 1.5 0 011.06.44L8.71 3H13a1.5 1.5 0 011.5 1.5v8A1.5 1.5 0 0113 14H3a1.5 1.5 0 01-1.5-1.5V3z" fill="#B07CDB"/>
            <path d="M1.5 5.5h13v7a1.5 1.5 0 01-1.5 1.5H3a1.5 1.5 0 01-1.5-1.5v-7z" fill="#9B59B6"/>
            <path d="M1.5 5.5h13v1H1.5z" fill="rgba(0,0,0,0.08)"/>
            <circle cx="8" cy="9.5" r="2.5" fill="none" stroke="rgba(255,255,255,0.5)" strokeWidth="0.8"/>
            <path d="M8 7.8l.4-.7.8.1.5-.6.8.3.2-.8h.8l-.1.8.7.4-.3.7.6.5-.6.5.3.7-.7.4.1.8h-.8l-.2-.8-.8.3-.5-.6-.8.1L8 9.5" fill="rgba(255,255,255,0.35)"/>
            <path d="M8 7.8l-.4-.7-.8.1-.5-.6-.8.3-.2-.8h-.8l.1.8-.7.4.3.7-.6.5.6.5-.3.7.7.4-.1.8h.8l.2-.8.8.3.5-.6.8.1L8 9.5" fill="rgba(255,255,255,0.35)"/>
            <circle cx="8" cy="9.5" r="1" fill="rgba(255,255,255,0.5)"/>
          </svg>
        </span>
        <span className="root-card-name" title={folder.name}>{folder.name}</span>
        <button
          type="button"
          className="root-card-delete"
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
          title="Delete smart folder"
          aria-label={`Delete ${folder.name}`}
        >&times;</button>
      </div>
      <div className="root-card-meta">
        <span title={folder.query}>{folder.query}</span>
      </div>
    </div>
  );
}
