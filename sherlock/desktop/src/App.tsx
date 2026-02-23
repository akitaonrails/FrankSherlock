import { useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cleanupOllamaModels, ensureDatabase, removeRoot } from "./api";
import type {
  DbStats,
  RootInfo,
  RuntimeStatus,
  SearchItem,
  SetupStatus
} from "./types";
import { errorMessage } from "./utils";
import Titlebar from "./components/Titlebar/Titlebar";
import Sidebar from "./components/Sidebar/Sidebar";
import Content from "./components/Content/Content";
import StatusBar from "./components/StatusBar/StatusBar";
import ToastContainer from "./components/Toasts/ToastContainer";
import SetupModal from "./components/modals/SetupModal";
import ResumeModal from "./components/modals/ResumeModal";
import ScanSummaryModal from "./components/modals/ScanSummaryModal";
import PreviewModal from "./components/modals/PreviewModal";
import ConfirmDeleteModal from "./components/modals/ConfirmDeleteModal";
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
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [roots, setRoots] = useState<RootInfo[]>([]);
  const [selectedRootId, setSelectedRootId] = useState<number | null>(null);
  const [readOnly, setReadOnly] = useState(false);
  const [showResumeModal, setShowResumeModal] = useState(false);
  const [confirmDeleteRoot, setConfirmDeleteRoot] = useState<RootInfo | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);

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
      {confirmDeleteRoot && (
        <ConfirmDeleteModal
          root={confirmDeleteRoot}
          onCancel={() => setConfirmDeleteRoot(null)}
          onConfirm={onDeleteRoot}
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
