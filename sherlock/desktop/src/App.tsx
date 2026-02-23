import { useCallback, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cleanupOllamaModels, copyFilesToClipboard, deleteFiles, ensureDatabase, removeRoot, renameFile } from "./api";
import type {
  DbStats,
  RootInfo,
  RuntimeStatus,
  SearchItem,
  SetupStatus,
  SortField,
  SortOrder,
} from "./types";
import { errorMessage } from "./utils";
import { fileName } from "./utils/format";
import Titlebar from "./components/Titlebar/Titlebar";
import Sidebar from "./components/Sidebar/Sidebar";
import Content from "./components/Content/Content";
import ContextMenu from "./components/Content/ContextMenu";
import StatusBar from "./components/StatusBar/StatusBar";
import ToastContainer from "./components/Toasts/ToastContainer";
import SetupModal from "./components/modals/SetupModal";
import ResumeModal from "./components/modals/ResumeModal";
import ScanSummaryModal from "./components/modals/ScanSummaryModal";
import PreviewModal from "./components/modals/PreviewModal";
import ConfirmDeleteModal from "./components/modals/ConfirmDeleteModal";
import ConfirmFileDeleteModal from "./components/modals/ConfirmFileDeleteModal";
import RenameModal from "./components/modals/RenameModal";
import HelpModal from "./components/modals/HelpModal";
import { useToast } from "./hooks/useToast";
import { useUserConfig } from "./hooks/useUserConfig";
import { useGridColumns } from "./hooks/useGridColumns";
import { useInfiniteScroll } from "./hooks/useInfiniteScroll";
import { usePolling } from "./hooks/usePolling";
import { useSelection } from "./hooks/useSelection";
import { useSearch } from "./hooks/useSearch";
import { useScanManager } from "./hooks/useScanManager";
import { useGridNavigation } from "./hooks/useGridNavigation";
import { useAppInit } from "./hooks/useAppInit";
import "./app.css";

const POLL_MS = 1200;

export default function App() {
  /* ── Shared state ── */
  const [query, setQuery] = useState("");
  const [selectedMediaType, setSelectedMediaType] = useState("");
  const [sortBy, setSortBy] = useState<SortField>("dateModified");
  const [sortOrder, setSortOrder] = useState<SortOrder>("desc");
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [roots, setRoots] = useState<RootInfo[]>([]);
  const [selectedRootId, setSelectedRootId] = useState<number | null>(null);
  const [readOnly, setReadOnly] = useState(false);
  const [showResumeModal, setShowResumeModal] = useState(false);
  const [confirmDeleteRoot, setConfirmDeleteRoot] = useState<RootInfo | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [confirmDeleteFiles, setConfirmDeleteFiles] = useState<SearchItem[] | null>(null);
  const [renameItem, setRenameItem] = useState<SearchItem | null>(null);

  /* ── Refs ── */
  const sentinelRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);

  /* ── Hooks ── */
  const { notice, error, setNotice, setError } = useToast();
  useUserConfig();
  const columnsRef = useGridColumns(gridRef);

  const {
    selectedIndices, focusIndex, anchorIndex,
    selectOnly, toggleSelect, rangeSelect, selectAll, clearSelection,
  } = useSelection();

  const {
    items, total, loading, loadingMore, canLoadMore, runSearch, onLoadMore,
  } = useSearch({
    query,
    selectedMediaType,
    selectedRootId,
    sortBy,
    sortOrder,
    isReady: !setup || setup.isReady,
    onClearSelection: clearSelection,
  });

  const scanManager = useScanManager({
    setSetup,
    setRuntime,
    setDbStats,
    setRoots,
    setReadOnly,
    setShowResumeModal,
    setNotice,
    setError,
    runSearch,
    itemsLength: () => items.length,
  });

  useAppInit(scanManager.initApp);
  usePolling(POLL_MS, scanManager.pollRuntimeAndScans, [scanManager.trackedJobIds]);
  useInfiniteScroll(sentinelRef, onLoadMore, [items.length, total, loadingMore]);

  const hasModalOpen = !!(confirmDeleteFiles || renameItem);

  const onRequestDelete = useCallback(() => {
    const filesToDelete = [...selectedIndices].sort((a, b) => a - b)
      .filter(i => i < items.length)
      .map(i => items[i]);
    if (filesToDelete.length > 0) setConfirmDeleteFiles(filesToDelete);
  }, [selectedIndices, items]);

  const onRequestRename = useCallback(() => {
    if (selectedIndices.size !== 1) return;
    const idx = [...selectedIndices][0];
    if (idx < items.length) setRenameItem(items[idx]);
  }, [selectedIndices, items]);

  useGridNavigation({
    items,
    selectedIndices,
    focusIndex,
    anchorIndex,
    columnsRef,
    gridRef,
    previewOpen,
    showSummary: scanManager.trackedJobIds.length === 0 && scanManager.completedJobs.length > 0,
    showResumeModal,
    confirmDeleteRoot,
    setup,
    canLoadMore,
    hasModalOpen,
    selectOnly,
    rangeSelect,
    selectAll,
    clearSelection,
    setPreviewOpen,
    setCompletedJobs: scanManager.setCompletedJobs,
    setShowResumeModal,
    setConfirmDeleteRoot,
    setNotice,
    onLoadMore,
    showHelp,
    setShowHelp,
    onRequestDelete,
    onRequestRename,
  });

  /* ── Derived values ── */
  const mediaTypeOptions = useMemo(
    () => ["", "document", "anime", "screenshot", "photo", "artwork", "manga", "other"],
    []
  );

  const isScanning = scanManager.activeScans.some((s) => s.status === "running");
  const runningScansCount = scanManager.activeScans.filter((s) => s.status === "running").length;
  const interruptedScans = scanManager.activeScans.filter((s) => s.status === "interrupted");
  const showSummary = scanManager.trackedJobIds.length === 0 && scanManager.completedJobs.length > 0;

  const previewItems: SearchItem[] = previewOpen
    ? [...selectedIndices].sort((a, b) => a - b).slice(0, 4).filter(i => i < items.length).map(i => items[i])
    : [];
  const singlePreviewIndex = selectedIndices.size === 1 ? [...selectedIndices][0] : null;

  /* ── Handlers ── */
  function onWindowClose() {
    cleanupOllamaModels().catch(() => {});
    void getCurrentWindow().destroy();
  }

  async function onDeleteRoot(root: RootInfo) {
    if (readOnly) return;
    setConfirmDeleteRoot(null);
    try {
      const result = await removeRoot(root.id);
      if (selectedRootId === root.id) setSelectedRootId(null);
      setNotice(`Removed "${root.rootName}": ${result.filesRemoved} files purged.`);
      await scanManager.refreshRoots();
      const stats = await ensureDatabase();
      setDbStats(stats);
      await runSearch(0, false);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function onTileClick(idx: number, e: React.MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      toggleSelect(idx);
    } else if (e.shiftKey && anchorIndex != null) {
      rangeSelect(anchorIndex, idx);
    } else {
      selectOnly(idx);
    }
  }

  function onTileDoubleClick(idx: number) {
    selectOnly(idx);
    setPreviewOpen(true);
  }

  function onTileContextMenu(idx: number, e: React.MouseEvent) {
    e.preventDefault();
    // If tile is not in selection, select it first
    if (!selectedIndices.has(idx)) selectOnly(idx);
    setContextMenu({ x: e.clientX, y: e.clientY });
  }

  function handleContextCopy() {
    setContextMenu(null);
    const paths = [...selectedIndices].sort((a, b) => a - b)
      .filter(i => i < items.length)
      .map(i => items[i].absPath);
    if (paths.length > 0) {
      copyFilesToClipboard(paths).catch(() => {});
      setNotice(`Copied ${paths.length} file path(s)`);
    }
  }

  function handleContextRename() {
    setContextMenu(null);
    onRequestRename();
  }

  function handleContextDelete() {
    setContextMenu(null);
    onRequestDelete();
  }

  async function handleDeleteFiles() {
    if (!confirmDeleteFiles) return;
    const ids = confirmDeleteFiles.map(f => f.id);
    setConfirmDeleteFiles(null);
    try {
      const result = await deleteFiles(ids);
      clearSelection();
      await runSearch(0, false);
      const stats = await ensureDatabase();
      setDbStats(stats);
      setNotice(`Deleted ${result.deletedCount} file(s)`);
      if (result.errors.length > 0) {
        setError(`Some files had errors: ${result.errors[0]}`);
      }
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleRenameFile(newName: string) {
    if (!renameItem) return;
    const item = renameItem;
    setRenameItem(null);
    try {
      await renameFile(item.id, newName);
      clearSelection();
      await runSearch(0, false);
      setNotice(`Renamed to "${newName}"`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  /* ── JSX ── */
  return (
    <div className="app-shell">
      {/* ── Modals ── */}
      {setup && !setup.isReady && (
        <SetupModal setup={setup} onRecheck={scanManager.onRecheckSetup} onDownload={scanManager.onSetupDownload} />
      )}
      {showResumeModal && (
        <ResumeModal
          interruptedScans={interruptedScans}
          onDismiss={() => setShowResumeModal(false)}
          onResumeAll={scanManager.onResumeAllInterrupted}
        />
      )}
      {showSummary && (
        <ScanSummaryModal completedJobs={scanManager.completedJobs} onClose={() => scanManager.setCompletedJobs([])} />
      )}
      {previewItems.length > 0 && (
        <PreviewModal
          previewItems={previewItems}
          selectedCount={selectedIndices.size}
          singlePreviewIndex={singlePreviewIndex}
          totalItems={items.length}
          onClose={() => setPreviewOpen(false)}
          onNavigate={(idx) => { selectOnly(idx); }}
        />
      )}
      {showHelp && <HelpModal onClose={() => setShowHelp(false)} />}
      {confirmDeleteRoot && (
        <ConfirmDeleteModal
          root={confirmDeleteRoot}
          onCancel={() => setConfirmDeleteRoot(null)}
          onConfirm={onDeleteRoot}
        />
      )}
      {confirmDeleteFiles && (
        <ConfirmFileDeleteModal
          files={confirmDeleteFiles}
          onCancel={() => setConfirmDeleteFiles(null)}
          onConfirm={handleDeleteFiles}
        />
      )}
      {renameItem && (
        <RenameModal
          currentName={fileName(renameItem.relPath)}
          onCancel={() => setRenameItem(null)}
          onConfirm={handleRenameFile}
        />
      )}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          selectedCount={selectedIndices.size}
          onCopy={handleContextCopy}
          onRename={handleContextRename}
          onDelete={handleContextDelete}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* ── Titlebar ── */}
      <Titlebar onClose={onWindowClose} />

      {/* ── Read-Only Banner ── */}
      {readOnly && (
        <div className="readonly-banner">
          Read-only mode — database cannot be modified
        </div>
      )}

      {/* ── Main Area ── */}
      <div className="main-area">
        <Sidebar
          roots={roots}
          selectedRootId={selectedRootId}
          activeScans={scanManager.activeScans}
          runtime={runtime}
          dbStats={dbStats}
          readOnly={readOnly}
          setupReady={setup ? setup.isReady : false}
          isScanning={isScanning}
          onSelectRoot={setSelectedRootId}
          onDeleteRoot={(root) => setConfirmDeleteRoot(root)}
          onPickAndScan={() => scanManager.onPickAndScan(setup, readOnly)}
          onCancelScan={(scan) => scanManager.onCancelScan(scan, readOnly)}
          onResumeScan={(scan) => scanManager.onResumeScan(scan, readOnly)}
          onCleanupOllama={scanManager.onCleanupOllama}
        />

        <Content
          query={query}
          onQueryChange={setQuery}
          selectedMediaType={selectedMediaType}
          onMediaTypeChange={setSelectedMediaType}
          mediaTypeOptions={mediaTypeOptions}
          sortBy={sortBy}
          onSortByChange={setSortBy}
          sortOrder={sortOrder}
          onSortOrderChange={setSortOrder}
          hasTextQuery={query.trim().length > 0}
          items={items}
          total={total}
          loading={loading}
          loadingMore={loadingMore}
          canLoadMore={canLoadMore}
          isScanning={isScanning}
          selectedRootName={selectedRootId != null ? (roots.find((r) => r.id === selectedRootId)?.rootName ?? null) : null}
          selectedIndices={selectedIndices}
          focusIndex={focusIndex}
          gridRef={gridRef}
          sentinelRef={sentinelRef}
          onTileClick={onTileClick}
          onTileDoubleClick={onTileDoubleClick}
          onTileContextMenu={onTileContextMenu}
        />
      </div>

      {/* ── Status Bar ── */}
      <StatusBar
        runtime={runtime}
        dbStats={dbStats}
        isScanning={isScanning}
        runningScansCount={runningScansCount}
        selectedCount={selectedIndices.size}
      />

      {/* ── Toasts ── */}
      <ToastContainer notice={notice} error={error} />
    </div>
  );
}
