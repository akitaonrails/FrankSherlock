import type { RefObject } from "react";
import type { SearchItem, SortField, SortOrder } from "../../types";
import Toolbar from "./Toolbar";
import ResultsMeta from "./ResultsMeta";
import ImageGrid from "./ImageGrid";
import "./Content.css";

type ContentProps = {
  query: string;
  onQueryChange: (q: string) => void;
  selectedMediaType: string;
  onMediaTypeChange: (t: string) => void;
  mediaTypeOptions: string[];
  sortBy: SortField;
  onSortByChange: (v: SortField) => void;
  sortOrder: SortOrder;
  onSortOrderChange: (v: SortOrder) => void;
  hasTextQuery: boolean;
  items: SearchItem[];
  total: number;
  loading: boolean;
  loadingMore: boolean;
  canLoadMore: boolean;
  isScanning: boolean;
  selectedRootName: string | null;
  selectedIndices: Set<number>;
  focusIndex: number | null;
  gridRef: RefObject<HTMLDivElement>;
  sentinelRef: RefObject<HTMLDivElement>;
  onTileClick: (idx: number, e: React.MouseEvent) => void;
  onTileDoubleClick: (idx: number) => void;
  onTileContextMenu: (idx: number, e: React.MouseEvent) => void;
};

export default function Content({
  query, onQueryChange, selectedMediaType, onMediaTypeChange, mediaTypeOptions,
  sortBy, onSortByChange, sortOrder, onSortOrderChange, hasTextQuery,
  items, total, loading, loadingMore, canLoadMore, isScanning, selectedRootName,
  selectedIndices, focusIndex, gridRef, sentinelRef, onTileClick, onTileDoubleClick,
  onTileContextMenu,
}: ContentProps) {
  return (
    <div className="content">
      <Toolbar
        query={query}
        onQueryChange={onQueryChange}
        selectedMediaType={selectedMediaType}
        onMediaTypeChange={onMediaTypeChange}
        mediaTypeOptions={mediaTypeOptions}
        sortBy={sortBy}
        onSortByChange={onSortByChange}
        sortOrder={sortOrder}
        onSortOrderChange={onSortOrderChange}
        hasTextQuery={hasTextQuery}
      />

      <div className="content-body">
        <ResultsMeta
          count={items.length}
          total={total}
          loading={loading}
          isScanning={isScanning}
          selectedRootName={selectedRootName}
        />

        <ImageGrid
          items={items}
          selectedIndices={selectedIndices}
          focusIndex={focusIndex}
          gridRef={gridRef}
          onTileClick={onTileClick}
          onTileDoubleClick={onTileDoubleClick}
          onTileContextMenu={onTileContextMenu}
        />

        {canLoadMore && (
          <div ref={sentinelRef} className="load-sentinel">
            {loadingMore && <span>Loading...</span>}
          </div>
        )}
      </div>
    </div>
  );
}
