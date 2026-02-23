import type { RefObject } from "react";
import type { SearchItem } from "../../types";
import ImageTile from "./ImageTile";

type ImageGridProps = {
  items: SearchItem[];
  selectedIndices: Set<number>;
  focusIndex: number | null;
  gridRef: RefObject<HTMLDivElement>;
  onTileClick: (idx: number, e: React.MouseEvent) => void;
  onTileDoubleClick: (idx: number) => void;
  onTileContextMenu: (idx: number, e: React.MouseEvent) => void;
};

export default function ImageGrid({
  items, selectedIndices, focusIndex, gridRef, onTileClick, onTileDoubleClick, onTileContextMenu,
}: ImageGridProps) {
  return (
    <div className="grid" role="list" ref={gridRef}>
      {items.map((item, idx) => (
        <ImageTile
          key={item.id}
          item={item}
          index={idx}
          isSelected={selectedIndices.has(idx)}
          isFocused={focusIndex === idx}
          onClick={onTileClick}
          onDoubleClick={onTileDoubleClick}
          onContextMenu={onTileContextMenu}
        />
      ))}
    </div>
  );
}
