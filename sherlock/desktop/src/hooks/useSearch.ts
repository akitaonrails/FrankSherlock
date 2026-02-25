import { useEffect, useRef, useState, useCallback } from "react";
import { searchImages } from "../api";
import type { SearchItem, SearchResponse, SortField, SortOrder } from "../types";

const PAGE_SIZE = 80;

type UseSearchParams = {
  query: string;
  selectedMediaType: string;
  selectedRootId: number | null;
  sortBy: SortField;
  sortOrder: SortOrder;
  isReady: boolean;
  onClearSelection: () => void;
  onReconcileSelection: (oldItems: SearchItem[], newItems: SearchItem[]) => void;
};

export function useSearch({ query, selectedMediaType, selectedRootId, sortBy, sortOrder, isReady, onClearSelection, onReconcileSelection }: UseSearchParams) {
  const [items, setItems] = useState<SearchItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const requestIdRef = useRef(0);
  const itemsRef = useRef<SearchItem[]>([]);

  // Keep ref in sync for use in applySearchResponse
  itemsRef.current = items;

  const canLoadMore = items.length < total;

  function applySearchResponse(response: SearchResponse, append: boolean, preserveSelection: boolean) {
    setTotal(response.total);
    if (append) {
      setItems((prev) => [...prev, ...response.items]);
    } else {
      const oldItems = itemsRef.current;
      setItems(response.items);
      if (preserveSelection) {
        onReconcileSelection(oldItems, response.items);
      } else {
        onClearSelection();
      }
    }
  }

  const runSearch = useCallback(async (offset: number, append: boolean, limitOverride?: number, preserveSelection?: boolean) => {
    const reqId = ++requestIdRef.current;
    if (append) setLoadingMore(true);
    else setLoading(true);
    try {
      const response = await searchImages({
        query,
        limit: limitOverride ?? PAGE_SIZE,
        offset,
        mediaTypes: selectedMediaType ? [selectedMediaType] : undefined,
        rootScope: selectedRootId ? [selectedRootId] : undefined,
        sortBy,
        sortOrder,
      });
      if (reqId !== requestIdRef.current) return;
      applySearchResponse(response, append, preserveSelection ?? false);
    } catch (err) {
      if (reqId !== requestIdRef.current) return;
    } finally {
      if (reqId !== requestIdRef.current) return;
      setLoading(false);
      setLoadingMore(false);
    }
  }, [query, selectedMediaType, selectedRootId, sortBy, sortOrder, onClearSelection, onReconcileSelection]);

  const onLoadMore = useCallback(async () => {
    if (!canLoadMore || loadingMore) return;
    await runSearch(items.length, true);
  }, [canLoadMore, loadingMore, items.length, runSearch]);

  // Debounced search effect
  useEffect(() => {
    if (!isReady) return;
    const timer = setTimeout(() => {
      void runSearch(0, false);
    }, 260);
    return () => clearTimeout(timer);
  }, [query, selectedMediaType, selectedRootId, sortBy, sortOrder, isReady]);

  return { items, total, loading, loadingMore, canLoadMore, runSearch, onLoadMore };
}
