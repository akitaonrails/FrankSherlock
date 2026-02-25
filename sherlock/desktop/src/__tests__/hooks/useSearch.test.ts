import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useSearch } from "../../hooks/useSearch";
import { searchImages } from "../../api";

vi.mock("../../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api")>();
  return {
    ...actual,
    searchImages: vi.fn(),
  };
});

describe("useSearch", () => {
  const defaultParams = {
    query: "",
    selectedMediaType: "",
    selectedRootId: null,
    sortBy: "dateModified" as const,
    sortOrder: "desc" as const,
    isReady: true,
    onClearSelection: vi.fn(),
    onReconcileSelection: vi.fn(),
  };

  beforeEach(() => {
    vi.mocked(searchImages).mockReset();
    vi.mocked(searchImages).mockResolvedValue({
      total: 5,
      limit: 80,
      offset: 0,
      items: [
        { id: 1, rootId: 1, relPath: "a.jpg", absPath: "/a.jpg", mediaType: "photo", description: "", confidence: 0.9, mtimeNs: 0, sizeBytes: 100 },
      ],
      parsedQuery: { rawQuery: "", queryText: "", mediaTypes: [], parserConfidence: 1 },
    });
  });

  it("starts with empty items", () => {
    const { result } = renderHook(() => useSearch(defaultParams));
    expect(result.current.items).toEqual([]);
    expect(result.current.total).toBe(0);
  });

  it("loads items after debounce", async () => {
    const { result } = renderHook(() => useSearch(defaultParams));
    await waitFor(() => expect(result.current.items.length).toBe(1));
    expect(result.current.total).toBe(5);
  });

  it("does not search when not ready", async () => {
    vi.useFakeTimers();
    renderHook(() => useSearch({ ...defaultParams, isReady: false }));
    vi.advanceTimersByTime(500);
    expect(searchImages).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("passes sort params to searchImages", async () => {
    renderHook(() => useSearch({ ...defaultParams, sortBy: "name", sortOrder: "asc" }));
    await waitFor(() => expect(searchImages).toHaveBeenCalled());
    expect(searchImages).toHaveBeenCalledWith(
      expect.objectContaining({
        sortBy: "name",
        sortOrder: "asc",
      })
    );
  });
});
