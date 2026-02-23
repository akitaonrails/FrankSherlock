import { useCallback, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  addFilesToAlbum, copyFilesToClipboard, createAlbum, createSmartFolder,
  deleteAlbum, deleteFiles, deleteSmartFolder, ensureDatabase, getCliFolderPath,
  getFileMetadata, listAlbums, listRoots, listSmartFolders, removeRoot,
  renameFile, startScan, updateFileMetadata,
} from "./api";
import type {
  Album,
  DbStats,
  FileMetadata,
  RootInfo,
  RuntimeStatus,
  SearchItem,
  SetupStatus,
  SmartFolder,
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
import EditMetadataModal from "./components/modals/EditMetadataModal";
import ModelInfoModal from "./components/modals/ModelInfoModal";
import CreateAlbumModal from "./components/modals/CreateAlbumModal";
import CreateSmartFolderModal from "./components/modals/CreateSmartFolderModal";
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
  const [contextMenuMeta, setContextMenuMeta] = useState<{ description: string; extractedText: string } | null>(null);
  const [confirmDeleteFiles, setConfirmDeleteFiles] = useState<SearchItem[] | null>(null);
  const [renameItem, setRenameItem] = useState<SearchItem | null>(null);
  const [showModelInfo, setShowModelInfo] = useState(false);
  const [editMetadataItem, setEditMetadataItem] = useState<SearchItem | null>(null);
  const [forceShowSetup, setForceShowSetup] = useState(false); // F10 debug toggle
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  /* ── Album & Smart Folder state ── */
  const [albums, setAlbums] = useState<Album[]>([]);
  const [smartFolders, setSmartFolders] = useState<SmartFolder[]>([]);
  const [showCreateAlbum, setShowCreateAlbum] = useState(false);
  const [showCreateSmartFolder, setShowCreateSmartFolder] = useState(false);
  const [pendingAlbumFileIds, setPendingAlbumFileIds] = useState<number[]>([]);
  const [activeSmartFolderId, setActiveSmartFolderId] = useState<number | null>(null);

  /* ── Refs ── */
  const sentinelRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);

  /* ── Hooks ── */
  const { notice, error, setNotice, setError } = useToast();
  useUserConfig();
  const columnsRef = useGridColumns(gridRef);

  const {
    selectedIndices, focusIndex, anchorIndex,
    selectOnly, toggleSelect, rangeSelect, selectAll, clearSelection, replaceSelection,
  } = useSelection();

  const onReconcileSelection = useCallback((oldItems: SearchItem[], newItems: SearchItem[]) => {
    if (selectedIndices.size === 0) return;
    const selectedIds = new Set<number>();
    let focusId: number | null = null;
    let anchorId: number | null = null;
    for (const idx of selectedIndices) {
      if (idx < oldItems.length) {
        selectedIds.add(oldItems[idx].id);
      }
    }
    if (focusIndex !== null && focusIndex < oldItems.length) {
      focusId = oldItems[focusIndex].id;
    }
    if (anchorIndex !== null && anchorIndex < oldItems.length) {
      anchorId = oldItems[anchorIndex].id;
    }
    const newSelection = new Set<number>();
    let newFocus: number | null = null;
    let newAnchor: number | null = null;
    for (let i = 0; i < newItems.length; i++) {
      const id = newItems[i].id;
      if (selectedIds.has(id)) newSelection.add(i);
      if (id === focusId) newFocus = i;
      if (id === anchorId) newAnchor = i;
    }
    replaceSelection(newSelection, newFocus, newAnchor);
  }, [selectedIndices, focusIndex, anchorIndex, replaceSelection]);

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
    onReconcileSelection,
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

  /* ── Refresh helpers ── */
  async function refreshAlbums() {
    try { setAlbums(await listAlbums()); } catch { /* ignore */ }
  }
  async function refreshSmartFolders() {
    try { setSmartFolders(await listSmartFolders()); } catch { /* ignore */ }
  }

  /* ── Extended initApp: also load albums + smart folders + CLI folder ── */
  const initApp = useCallback(async () => {
    const result = await scanManager.initApp();
    try { setAlbums(await listAlbums()); } catch { /* ignore */ }
    try { setSmartFolders(await listSmartFolders()); } catch { /* ignore */ }

    // Handle CLI folder argument
    if (!result) return;
    const cliPath = await getCliFolderPath();
    if (!cliPath) return;

    const { roots: loadedRoots, scans, setupStatus, readOnly: isReadOnly } = result;
    const matchingRoot = loadedRoots.find((r) => r.rootPath === cliPath);
    const activeOrInterrupted = scans.filter(
      (s) => s.status === "running" || s.status === "interrupted",
    );

    if (matchingRoot) {
      // Root already exists — select it
      setSelectedRootId(matchingRoot.id);

      // Check for an interrupted scan on this specific root and resume it
      const interruptedForRoot = activeOrInterrupted.find(
        (s) => s.rootPath === cliPath && s.status === "interrupted",
      );
      if (interruptedForRoot && !isReadOnly && setupStatus.isReady) {
        try {
          const job = await startScan(cliPath);
          scanManager.addTrackedJobId(job.id);
        } catch { /* ignore — setup might not be ready */ }
      }
    } else if (!isReadOnly && setupStatus.isReady) {
      // New folder — check if there are already running/interrupted scans
      const hasOtherActive = activeOrInterrupted.length > 0;
      if (hasOtherActive) {
        // Other scans in progress — just add root (via startScan) without
        // blocking, but still select it. The scan will queue alongside others.
        // Actually, if other scans are interrupted (not running), we still
        // start the new scan — interrupted scans need user action to resume.
        const hasRunning = activeOrInterrupted.some((s) => s.status === "running");
        if (hasRunning) {
          // There are running scans — defer this one. Just notify user.
          // We can't add the root without scanning, so skip for now.
          return;
        }
      }
      // No running scans (or only interrupted ones) — start scan for new folder
      try {
        const job = await startScan(cliPath);
        scanManager.addTrackedJobId(job.id);
        await scanManager.refreshRoots();
        const updatedRoots = await listRoots();
        const newRoot = updatedRoots.find((r) => r.rootPath === cliPath);
        if (newRoot) setSelectedRootId(newRoot.id);
      } catch { /* ignore — setup might not be ready */ }
    }
  }, [scanManager.initApp]);

  useAppInit(initApp);
  usePolling(POLL_MS, scanManager.pollRuntimeAndScans, [scanManager.trackedJobIds]);
  useInfiniteScroll(sentinelRef, onLoadMore, [items.length, total, loadingMore]);

  const hasModalOpen = !!(confirmDeleteFiles || renameItem || editMetadataItem || showCreateAlbum || showCreateSmartFolder);

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
    forceShowSetup,
    setForceShowSetup,
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

  // Derive active album name from query (for sidebar highlight)
  const activeAlbumName = useMemo(() => {
    const match = query.match(/^album:(?:"([^"]+)"|(\S+))$/i);
    return match ? (match[1] ?? match[2] ?? null) : null;
  }, [query]);

  // Derive subtitle for titlebar based on current context
  const subtitle = useMemo(() => {
    if (activeAlbumName) return activeAlbumName;
    const sf = smartFolders.find(f => f.id === activeSmartFolderId);
    if (sf) return sf.name;
    if (selectedRootId != null) {
      const root = roots.find(r => r.id === selectedRootId);
      if (root) return root.rootName;
    }
    return null;
  }, [activeAlbumName, activeSmartFolderId, smartFolders, selectedRootId, roots]);

  /* ── Handlers ── */
  function onWindowClose() {
    void getCurrentWindow().close();
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
    if (!selectedIndices.has(idx)) selectOnly(idx);
    setContextMenu({ x: e.clientX, y: e.clientY });

    // For single selection, fetch metadata (description + OCR text) for context menu
    const effectiveSelection = selectedIndices.has(idx) ? selectedIndices : new Set([idx]);
    if (effectiveSelection.size === 1) {
      const item = items[idx];
      if (item) {
        getFileMetadata(item.id)
          .then((meta) => setContextMenuMeta({ description: meta.description, extractedText: meta.extractedText }))
          .catch(() => setContextMenuMeta(null));
      }
    } else {
      setContextMenuMeta(null);
    }
  }

  function handleContextCopyPath() {
    setContextMenu(null);
    const paths = [...selectedIndices].sort((a, b) => a - b)
      .filter(i => i < items.length)
      .map(i => items[i].absPath);
    if (paths.length > 0) {
      copyFilesToClipboard(paths).catch(() => {});
      setNotice(`Copied ${paths.length} file path(s)`);
    }
  }

  function handleContextCopyDescription() {
    setContextMenu(null);
    if (!contextMenuMeta?.description) return;
    copyFilesToClipboard([contextMenuMeta.description]).catch(() => {});
    setNotice("Copied description");
  }

  function handleContextCopyOcrText() {
    setContextMenu(null);
    if (!contextMenuMeta?.extractedText) return;
    copyFilesToClipboard([contextMenuMeta.extractedText]).catch(() => {});
    setNotice("Copied OCR text");
  }

  function handleContextRename() {
    setContextMenu(null);
    onRequestRename();
  }

  function handleContextDelete() {
    setContextMenu(null);
    onRequestDelete();
  }

  function handleContextEditMetadata() {
    setContextMenu(null);
    if (selectedIndices.size !== 1) return;
    const idx = [...selectedIndices][0];
    if (idx < items.length) setEditMetadataItem(items[idx]);
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

  async function handleSaveMetadata(data: FileMetadata) {
    setEditMetadataItem(null);
    try {
      await updateFileMetadata(
        data.id,
        data.mediaType,
        data.description,
        data.extractedText,
        data.canonicalMentions,
        data.locationText,
      );
      await runSearch(0, false);
      setNotice("Metadata updated");
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  /* ── Album handlers ── */
  function handleSelectAlbum(album: Album) {
    const q = album.name.includes(" ") ? `album:"${album.name}"` : `album:${album.name}`;
    setQuery(q);
    setActiveSmartFolderId(null);
  }

  async function handleDeleteAlbum(album: Album) {
    try {
      await deleteAlbum(album.id);
      await refreshAlbums();
      setNotice(`Deleted album "${album.name}"`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleAddToAlbum(albumId: number) {
    setContextMenu(null);
    const fileIds = [...selectedIndices].sort((a, b) => a - b)
      .filter(i => i < items.length)
      .map(i => items[i].id);
    if (fileIds.length === 0) return;
    try {
      const added = await addFilesToAlbum(albumId, fileIds);
      await refreshAlbums();
      setNotice(`Added ${added} file(s) to album`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function handleCreateAlbumFromSelection() {
    setContextMenu(null);
    const fileIds = [...selectedIndices].sort((a, b) => a - b)
      .filter(i => i < items.length)
      .map(i => items[i].id);
    setPendingAlbumFileIds(fileIds);
    setShowCreateAlbum(true);
  }

  async function handleCreateAlbumConfirm(name: string) {
    setShowCreateAlbum(false);
    try {
      const album = await createAlbum(name);
      if (pendingAlbumFileIds.length > 0) {
        await addFilesToAlbum(album.id, pendingAlbumFileIds);
      }
      setPendingAlbumFileIds([]);
      await refreshAlbums();
      setNotice(`Created album "${name}"${pendingAlbumFileIds.length > 0 ? ` with ${pendingAlbumFileIds.length} file(s)` : ""}`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  /* ── Smart Folder handlers ── */
  function handleSelectSmartFolder(folder: SmartFolder) {
    setQuery(folder.query);
    setActiveSmartFolderId(folder.id);
  }

  async function handleDeleteSmartFolder(folder: SmartFolder) {
    try {
      await deleteSmartFolder(folder.id);
      await refreshSmartFolders();
      if (activeSmartFolderId === folder.id) setActiveSmartFolderId(null);
      setNotice(`Deleted smart folder "${folder.name}"`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleCreateSmartFolderConfirm(name: string) {
    setShowCreateSmartFolder(false);
    try {
      const folder = await createSmartFolder(name, query);
      await refreshSmartFolders();
      setActiveSmartFolderId(folder.id);
      setNotice(`Saved smart folder "${name}"`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  /* ── JSX ── */
  return (
    <div className="app-shell">
      {/* ── Modals ── */}
      {setup && (!setup.isReady || forceShowSetup) && (
        <SetupModal setup={setup} onRecheck={scanManager.onRecheckSetup} onDownload={scanManager.onSetupDownload} onClose={forceShowSetup ? () => setForceShowSetup(false) : undefined} />
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
      {showModelInfo && runtime && (
        <ModelInfoModal runtime={runtime} setup={setup} onClose={() => setShowModelInfo(false)} />
      )}
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
          albums={albums}
          description={contextMenuMeta?.description ?? null}
          extractedText={contextMenuMeta?.extractedText ?? null}
          onCopyPath={handleContextCopyPath}
          onCopyDescription={handleContextCopyDescription}
          onCopyOcrText={handleContextCopyOcrText}
          onRename={handleContextRename}
          onEditMetadata={handleContextEditMetadata}
          onDelete={handleContextDelete}
          onAddToAlbum={handleAddToAlbum}
          onCreateAlbumFromSelection={handleCreateAlbumFromSelection}
          onClose={() => setContextMenu(null)}
        />
      )}
      {editMetadataItem && (
        <EditMetadataModal
          fileId={editMetadataItem.id}
          onSave={handleSaveMetadata}
          onCancel={() => setEditMetadataItem(null)}
        />
      )}
      {showCreateAlbum && (
        <CreateAlbumModal
          onCancel={() => { setShowCreateAlbum(false); setPendingAlbumFileIds([]); }}
          onConfirm={handleCreateAlbumConfirm}
        />
      )}
      {showCreateSmartFolder && (
        <CreateSmartFolderModal
          query={query}
          onCancel={() => setShowCreateSmartFolder(false)}
          onConfirm={handleCreateSmartFolderConfirm}
        />
      )}

      {/* ── Titlebar ── */}
      <Titlebar
        onClose={onWindowClose}
        subtitle={subtitle}
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed(c => !c)}
      />

      {/* ── Read-Only Banner ── */}
      {readOnly && (
        <div className="readonly-banner">
          Read-only mode — database cannot be modified
        </div>
      )}

      {/* ── Main Area ── */}
      <div className={`main-area${sidebarCollapsed ? " sidebar-collapsed" : ""}`}>
        <Sidebar
          roots={roots}
          selectedRootId={selectedRootId}
          activeScans={scanManager.activeScans}
          dbStats={dbStats}
          readOnly={readOnly}
          setupReady={setup ? setup.isReady : false}
          albums={albums}
          smartFolders={smartFolders}
          activeAlbumName={activeAlbumName}
          activeSmartFolderId={activeSmartFolderId}
          onSelectRoot={setSelectedRootId}
          onDeleteRoot={(root) => setConfirmDeleteRoot(root)}
          onPickAndScan={() => scanManager.onPickAndScan(setup, readOnly)}
          onCancelScan={(scan) => scanManager.onCancelScan(scan, readOnly)}
          onResumeScan={(scan) => scanManager.onResumeScan(scan, readOnly)}
          onSelectAlbum={handleSelectAlbum}
          onDeleteAlbum={handleDeleteAlbum}
          onSelectSmartFolder={handleSelectSmartFolder}
          onDeleteSmartFolder={handleDeleteSmartFolder}
        />

        <Content
          query={query}
          onQueryChange={(q) => { setQuery(q); setActiveSmartFolderId(null); }}
          selectedMediaType={selectedMediaType}
          onMediaTypeChange={setSelectedMediaType}
          mediaTypeOptions={mediaTypeOptions}
          sortBy={sortBy}
          onSortByChange={setSortBy}
          sortOrder={sortOrder}
          onSortOrderChange={setSortOrder}
          hasTextQuery={query.trim().length > 0}
          onSaveSmartFolder={() => setShowCreateSmartFolder(true)}
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
        isScanning={isScanning}
        runningScansCount={runningScansCount}
        selectedCount={selectedIndices.size}
        onShowModelInfo={() => setShowModelInfo(true)}
      />

      {/* ── Toasts ── */}
      <ToastContainer notice={notice} error={error} />
    </div>
  );
}
