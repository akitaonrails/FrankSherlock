import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import {
  cancelScan,
  cleanupOllamaModels,
  ensureDatabase,
  getRuntimeStatus,
  getScanJob,
  getSetupStatus,
  listActiveScans,
  listRoots,
  loadUserConfig,
  removeRoot,
  saveUserConfig,
  searchImages,
  startScan,
  startSetupDownload
} from "./api";
import type {
  DbStats,
  RootInfo,
  RuntimeStatus,
  ScanJobStatus,
  SearchItem,
  SearchResponse,
  SetupStatus
} from "./types";

const PAGE_SIZE = 40;
const POLL_MS = 1200;
const appWindow = getCurrentWindow();

export default function App() {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<SearchItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [minConfidence, setMinConfidence] = useState(0);
  const [selectedMediaType, setSelectedMediaType] = useState("");
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [activeScans, setActiveScans] = useState<ScanJobStatus[]>([]);
  const [trackedJobId, setTrackedJobId] = useState<number | null>(null);
  const [latestJob, setLatestJob] = useState<ScanJobStatus | null>(null);
  const [previewItem, setPreviewItem] = useState<SearchItem | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [zoom, setZoom] = useState(1.25);
  const [roots, setRoots] = useState<RootInfo[]>([]);
  const [selectedRootId, setSelectedRootId] = useState<number | null>(null);
  const [confirmDeleteRoot, setConfirmDeleteRoot] = useState<RootInfo | null>(null);
  const requestIdRef = useRef(0);
  const configRef = useRef<Record<string, unknown>>({});
  const lastProcessedRef = useRef(0);

  const canLoadMore = items.length < total;
  const mediaTypeOptions = useMemo(
    () => ["", "document", "anime", "screenshot", "photo", "artwork", "manga", "other"],
    []
  );

  const currentScan =
    (trackedJobId ? activeScans.find((job) => job.id === trackedJobId) : null) ??
    activeScans[0] ??
    latestJob;

  const isScanning = activeScans.some((s) => s.status === "running");

  // Load user config (zoom) on mount
  useEffect(() => {
    let mounted = true;
    loadUserConfig()
      .then((cfg) => {
        if (!mounted) return;
        configRef.current = cfg;
        const savedZoom = typeof cfg.zoom === "number" ? cfg.zoom : 1.25;
        setZoom(Math.max(0.5, Math.min(3.0, savedZoom)));
      })
      .catch(() => {});
    return () => { mounted = false; };
  }, []);

  // Apply zoom to root font-size
  useEffect(() => {
    document.documentElement.style.fontSize = `${14 * zoom}px`;
  }, [zoom]);

  // Keyboard: Ctrl+Shift+= (zoom in), Ctrl+Shift+- (zoom out)
  useEffect(() => {
    function handleZoomKey(e: KeyboardEvent) {
      if (!e.ctrlKey || !e.shiftKey) return;
      if (e.key === "+" || e.key === "=") {
        e.preventDefault();
        setZoom((prev) => {
          const next = Math.min(3.0, +(prev + 0.1).toFixed(2));
          persistZoom(next);
          return next;
        });
      } else if (e.key === "-" || e.key === "_") {
        e.preventDefault();
        setZoom((prev) => {
          const next = Math.max(0.5, +(prev - 0.1).toFixed(2));
          persistZoom(next);
          return next;
        });
      }
    }
    window.addEventListener("keydown", handleZoomKey);
    return () => window.removeEventListener("keydown", handleZoomKey);
  }, []);

  function persistZoom(value: number) {
    const cfg = { ...configRef.current, zoom: value };
    configRef.current = cfg;
    saveUserConfig(cfg).catch(() => {});
  }

  const refreshRoots = useCallback(async () => {
    try {
      const r = await listRoots();
      setRoots(r);
    } catch {
      // Silently ignore — roots will refresh on next poll
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const [db, setupStatus, runtimeStatus, scans, rootList] = await Promise.all([
          ensureDatabase(),
          getSetupStatus(),
          getRuntimeStatus(),
          listActiveScans(),
          listRoots()
        ]);
        if (!mounted) return;
        setDbStats(db);
        setSetup(setupStatus);
        setRuntime(runtimeStatus);
        setActiveScans(scans);
        setRoots(rootList);
        if (scans.length > 0) {
          setTrackedJobId(scans[0].id);
        }
      } catch (err) {
        if (!mounted) return;
        setError(err instanceof Error ? err.message : String(err));
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    const timer = setInterval(() => {
      void pollRuntimeAndScans();
    }, POLL_MS);
    return () => clearInterval(timer);
  }, [trackedJobId]);

  useEffect(() => {
    if (setup && !setup.isReady) return;
    const timer = setTimeout(() => {
      void runSearch(0, false);
    }, 260);
    return () => clearTimeout(timer);
  }, [query, minConfidence, selectedMediaType, selectedRootId, setup?.isReady]);

  // Keyboard: Esc closes preview
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        if (confirmDeleteRoot) {
          setConfirmDeleteRoot(null);
        } else if (previewItem) {
          setPreviewItem(null);
        }
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [previewItem, confirmDeleteRoot]);

  // Auto-dismiss toasts
  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 6000);
    return () => clearTimeout(t);
  }, [notice]);

  useEffect(() => {
    if (!error) return;
    const t = setTimeout(() => setError(null), 10000);
    return () => clearTimeout(t);
  }, [error]);

  async function pollRuntimeAndScans() {
    try {
      const [setupStatus, runtimeStatus, scans, trackedJob] = await Promise.all([
        getSetupStatus(),
        getRuntimeStatus(),
        listActiveScans(),
        trackedJobId !== null ? getScanJob(trackedJobId) : Promise.resolve(null)
      ]);
      setSetup(setupStatus);
      setRuntime(runtimeStatus);
      setActiveScans(scans);

      if (trackedJob) {
        setLatestJob(trackedJob);

        // Live-refresh grid: re-run search when new files are processed
        if (trackedJob.status === "running" && trackedJob.processedFiles > lastProcessedRef.current) {
          lastProcessedRef.current = trackedJob.processedFiles;
          void runSearch(0, false);
          void refreshRoots();
        }

        if (trackedJob.status === "completed") {
          lastProcessedRef.current = 0;
          setNotice(
            `Scan completed: ${trackedJob.processedFiles}/${trackedJob.totalFiles} files processed.`
          );
          setTrackedJobId(null);
          const stats = await ensureDatabase();
          setDbStats(stats);
          await refreshRoots();
          await runSearch(0, false);
        } else if (trackedJob.status === "failed") {
          lastProcessedRef.current = 0;
          setError(trackedJob.errorText || "Scan failed.");
          setTrackedJobId(null);
        }
      } else if (trackedJobId !== null && scans.length > 0) {
        setTrackedJobId(scans[0].id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function runSearch(offset: number, append: boolean) {
    const reqId = ++requestIdRef.current;
    if (append) setLoadingMore(true);
    else setLoading(true);
    try {
      const response = await searchImages({
        query,
        limit: PAGE_SIZE,
        offset,
        minConfidence: minConfidence > 0 ? minConfidence : undefined,
        mediaTypes: selectedMediaType ? [selectedMediaType] : undefined,
        rootScope: selectedRootId ? [selectedRootId] : undefined
      });
      if (reqId !== requestIdRef.current) return;
      applySearchResponse(response, append);
    } catch (err) {
      if (reqId !== requestIdRef.current) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (reqId !== requestIdRef.current) return;
      setLoading(false);
      setLoadingMore(false);
    }
  }

  function applySearchResponse(response: SearchResponse, append: boolean) {
    setTotal(response.total);
    setItems((prev) => (append ? [...prev, ...response.items] : response.items));
  }

  async function onLoadMore() {
    if (!canLoadMore || loadingMore) return;
    await runSearch(items.length, true);
  }

  async function onPickAndScan() {
    if (setup && !setup.isReady) {
      setError("Setup is incomplete. Finish Ollama setup before starting scans.");
      return;
    }
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select folder to scan" });
      if (!selected) return;
      setError(null);
      setNotice(null);
      const job = await startScan(selected as string);
      setTrackedJobId(job.id);
      setLatestJob(job);
      lastProcessedRef.current = 0;
      setNotice(`Scan started for ${job.rootPath}`);
      await refreshRoots();
      await pollRuntimeAndScans();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onCancelScan() {
    if (!currentScan) return;
    try {
      await cancelScan(currentScan.id);
      setNotice("Cancelling scan...");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onResumeScan() {
    if (!currentScan) return;
    try {
      const job = await startScan(currentScan.rootPath);
      setTrackedJobId(job.id);
      setLatestJob(job);
      lastProcessedRef.current = 0;
      setNotice(`Resuming scan for ${job.rootPath}`);
      await pollRuntimeAndScans();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onDeleteRoot(root: RootInfo) {
    setConfirmDeleteRoot(null);
    try {
      const result = await removeRoot(root.id);
      if (selectedRootId === root.id) setSelectedRootId(null);
      setNotice(`Removed "${root.rootName}": ${result.filesRemoved} files purged.`);
      await refreshRoots();
      const stats = await ensureDatabase();
      setDbStats(stats);
      await runSearch(0, false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onCleanupOllama() {
    try {
      const result = await cleanupOllamaModels();
      setNotice(`Unloaded ${result.stoppedModels}/${result.runningModels} model(s).`);
      await pollRuntimeAndScans();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onSetupDownload() {
    try {
      setError(null);
      await startSetupDownload();
      await pollRuntimeAndScans();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onRecheckSetup() {
    await pollRuntimeAndScans();
  }

  function thumbnailSrc(item: SearchItem): string | null {
    if (item.thumbnailPath) {
      const src = convertFileSrc(item.thumbnailPath);
      console.log("[thumb_debug]", { id: item.id, thumbnailPath: item.thumbnailPath, src });
      return src;
    }
    console.log("[thumb_debug] NO thumb_path for", item.id, item.relPath);
    return null;
  }

  function scanForRoot(rootId: number): ScanJobStatus | undefined {
    return activeScans.find((s) => s.rootId === rootId && s.status === "running");
  }

  const scanProgress = currentScan?.totalFiles
    ? Math.min(100, (currentScan.processedFiles / Math.max(1, currentScan.totalFiles)) * 100)
    : 0;

  return (
    <div className="app-shell">
      {/* ── Setup Modal ── */}
      {setup && !setup.isReady && (
        <div className="modal-overlay" role="dialog" aria-modal="true">
          <div className="setup-modal">
            <h2>First-Time Setup</h2>
            <p>Sherlock needs local Ollama service and required model(s) before scanning.</p>
            <div className="setup-status-grid">
              <div>
                <strong>Ollama</strong>
                <p>{setup.ollamaAvailable ? "Running" : "Not detected"}</p>
              </div>
              <div>
                <strong>Required</strong>
                <p>{setup.requiredModels.join(", ")}</p>
              </div>
              <div>
                <strong>Missing</strong>
                <p>{setup.missingModels.length ? setup.missingModels.join(", ") : "None"}</p>
              </div>
            </div>
            <ul className="setup-instructions">
              {setup.instructions.map((instruction) => (
                <li key={instruction}>{instruction}</li>
              ))}
            </ul>
            <div className="progress-wrap">
              <progress value={setup.download.progressPct} max={100} />
              <span>{setup.download.progressPct.toFixed(1)}%</span>
            </div>
            <p className="setup-download-text">{setup.download.message}</p>
            <div className="setup-actions">
              <button type="button" onClick={onRecheckSetup}>Recheck</button>
              <button
                type="button"
                onClick={onSetupDownload}
                disabled={
                  !setup.ollamaAvailable ||
                  setup.missingModels.length === 0 ||
                  setup.download.status === "running"
                }
              >
                {setup.download.status === "running" ? "Downloading..." : "Download model"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Preview Modal ── */}
      {previewItem && (
        <div className="modal-overlay preview-overlay" onClick={() => setPreviewItem(null)} role="dialog" aria-modal="true">
          <div className="preview-modal" onClick={(e) => e.stopPropagation()}>
            <button className="preview-close" onClick={() => setPreviewItem(null)} type="button" aria-label="Close preview">
              &times;
            </button>
            <div className="preview-image-wrap">
              <img src={convertFileSrc(previewItem.absPath)} alt={previewItem.relPath} />
            </div>
            <div className="preview-info">
              <h3 title={previewItem.relPath}>{previewItem.relPath}</h3>
              <p className="preview-desc">{previewItem.description || "No description"}</p>
              <div className="preview-meta">
                <span className="badge">{previewItem.mediaType}</span>
                <span>Confidence: {previewItem.confidence.toFixed(2)}</span>
                <span>{(previewItem.sizeBytes / 1024).toFixed(0)} KB</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── Delete Root Confirmation ── */}
      {confirmDeleteRoot && (
        <div className="modal-overlay" onClick={() => setConfirmDeleteRoot(null)} role="dialog" aria-modal="true">
          <div className="confirm-modal" onClick={(e) => e.stopPropagation()}>
            <h3>Remove folder?</h3>
            <p>
              This will remove <strong>{confirmDeleteRoot.rootName}</strong> and
              all {confirmDeleteRoot.fileCount} indexed files from the database and cache.
            </p>
            <p className="confirm-path">{confirmDeleteRoot.rootPath}</p>
            <p className="confirm-note">Original files on disk will not be touched.</p>
            <div className="confirm-actions">
              <button type="button" onClick={() => setConfirmDeleteRoot(null)}>Cancel</button>
              <button type="button" className="danger-btn" onClick={() => onDeleteRoot(confirmDeleteRoot)}>
                Remove
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Titlebar ── */}
      <div className="titlebar" data-tauri-drag-region>
        <span>Frank Sherlock</span>
        <div className="titlebar-controls">
          <button type="button" onClick={() => appWindow.minimize()} aria-label="Minimize">&#x2500;</button>
          <button type="button" onClick={() => appWindow.toggleMaximize()} aria-label="Maximize">&#x25A1;</button>
          <button type="button" className="close" onClick={() => appWindow.close()} aria-label="Close">&#x2715;</button>
        </div>
      </div>

      {/* ── Main Area ── */}
      <div className={`main-area${sidebarOpen ? "" : " sidebar-collapsed"}`}>
        {/* ── Sidebar ── */}
        <aside className="sidebar">
          <div className="sidebar-section">
            <span>Folders</span>
            <button
              type="button"
              className="sidebar-add-btn"
              onClick={onPickAndScan}
              disabled={setup ? !setup.isReady : true}
              title="Add folder to scan"
            >+</button>
          </div>

          {roots.length === 0 && (
            <div className="sidebar-empty">No folders scanned yet</div>
          )}

          <div className="root-list">
            {roots.map((root) => {
              const scan = scanForRoot(root.id);
              const isSelected = selectedRootId === root.id;
              const progress = scan?.totalFiles
                ? Math.min(100, (scan.processedFiles / Math.max(1, scan.totalFiles)) * 100)
                : 0;
              return (
                <div
                  key={root.id}
                  className={`root-card${isSelected ? " selected" : ""}`}
                  onClick={() => setSelectedRootId(isSelected ? null : root.id)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setSelectedRootId(isSelected ? null : root.id);
                    }
                  }}
                >
                  <div className="root-card-header">
                    <span className="root-card-icon">&#128193;</span>
                    <span className="root-card-name" title={root.rootPath}>{root.rootName}</span>
                    <button
                      type="button"
                      className="root-card-delete"
                      onClick={(e) => { e.stopPropagation(); setConfirmDeleteRoot(root); }}
                      title="Remove folder"
                      aria-label={`Remove ${root.rootName}`}
                    >&times;</button>
                  </div>
                  <div className="root-card-meta">
                    <span>{root.fileCount.toLocaleString()} files</span>
                  </div>
                  {scan && (
                    <div className="root-card-scan">
                      <progress value={progress} max={100} />
                      <span>{scan.processedFiles}/{scan.totalFiles}</span>
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          {/* Scan status for non-root-specific display */}
          {currentScan && currentScan.status === "running" && (
            <div className="sidebar-scan-progress">
              <div className="sidebar-scan-progress-header">
                {currentScan.processedFiles} / {currentScan.totalFiles} ({scanProgress.toFixed(1)}%)
              </div>
              <progress value={scanProgress} max={100} />
              <div className="sidebar-scan-meta">
                +{currentScan.added} new, {currentScan.modified} mod, {currentScan.moved} moved
              </div>
              <button type="button" onClick={onCancelScan}>Cancel Scan</button>
            </div>
          )}
          {currentScan && currentScan.status === "interrupted" && (
            <div className="sidebar-scan-progress">
              <div>Interrupted at {currentScan.processedFiles} / {currentScan.totalFiles}</div>
              <button type="button" onClick={onResumeScan}>Resume Scan</button>
            </div>
          )}

          <div className="sidebar-spacer" />

          <div className="sidebar-section"><span>Info</span></div>
          <div className="sidebar-item">Files: <span>{dbStats?.files ?? "..."}</span></div>
          <div className="sidebar-item">Roots: <span>{dbStats?.roots ?? "..."}</span></div>

          <div className="sidebar-section"><span>Actions</span></div>
          <button type="button" className="sidebar-action-btn" onClick={onCleanupOllama}>Unload Models</button>
        </aside>

        {/* ── Content ── */}
        <div className="content">
          <div className="toolbar">
            <button
              type="button"
              className="toolbar-toggle"
              onClick={() => setSidebarOpen((v) => !v)}
              aria-label="Toggle sidebar"
            >
              &#9776;
            </button>
            <input
              type="search"
              placeholder="Search images..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              aria-label="Search query"
            />
            <select
              value={selectedMediaType}
              onChange={(e) => setSelectedMediaType(e.target.value)}
              aria-label="Media type filter"
            >
              {mediaTypeOptions.map((opt) => (
                <option key={opt} value={opt}>
                  {opt ? opt : "all types"}
                </option>
              ))}
            </select>
            <label className="confidence-wrap">
              Min conf
              <input
                type="number"
                min={0}
                max={1}
                step={0.05}
                value={minConfidence}
                onChange={(e) => setMinConfidence(Number(e.target.value))}
              />
            </label>
          </div>

          <div className="content-body">
            <div className="results-meta">
              <span>
                {items.length} of {total} results
                {selectedRootId != null && roots.length > 0 && (
                  <> in <strong>{roots.find((r) => r.id === selectedRootId)?.rootName ?? "..."}</strong></>
                )}
              </span>
              {loading && <span>Searching...</span>}
              {isScanning && <span className="scanning-indicator">Scanning...</span>}
            </div>

            <div className="grid" role="list">
              {items.map((item) => {
                const thumb = thumbnailSrc(item);
                return (
                  <article
                    key={item.id}
                    className="tile"
                    role="listitem"
                    tabIndex={0}
                    onClick={() => setPreviewItem(item)}
                    onKeyDown={(e) => {
                      if (e.key === " " || e.key === "Enter") {
                        e.preventDefault();
                        setPreviewItem(item);
                      }
                    }}
                  >
                    <div className="thumb">
                      {thumb ? (
                        <img
                          src={thumb}
                          alt={item.relPath}
                          loading="lazy"
                          onError={(e) => console.error("[thumb_error]", item.id, item.relPath, thumb, e)}
                        />
                      ) : (
                        <span className="badge">{item.mediaType}</span>
                      )}
                    </div>
                    <div className="tile-body">
                      <h3 title={item.relPath}>{item.relPath}</h3>
                      <p>{item.description || "No description yet"}</p>
                      <div className="tile-meta">
                        <span className="badge">{item.mediaType}</span>
                        <span>{item.confidence.toFixed(2)}</span>
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>

            {canLoadMore && (
              <div className="load-more">
                <button type="button" onClick={onLoadMore} disabled={loadingMore}>
                  {loadingMore ? "Loading..." : `Load more (${items.length} / ${total})`}
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* ── Status Bar ── */}
      <div className="statusbar">
        <span>
          VRAM:{" "}
          {runtime?.vramUsedMib != null && runtime?.vramTotalMib != null
            ? `${runtime.vramUsedMib}/${runtime.vramTotalMib} MiB`
            : "n/a"}
        </span>
        <span>Files: {dbStats?.files ?? "..."}</span>
        {isScanning && currentScan && (
          <span>
            Scanning: {currentScan.processedFiles}/{currentScan.totalFiles} ({scanProgress.toFixed(0)}%)
          </span>
        )}
        <span className="spacer" />
        <span>Model: {runtime?.currentModel || "none"}</span>
      </div>

      {/* ── Toasts ── */}
      <div className="toast-container">
        {notice && <div className="toast notice">{notice}</div>}
        {error && <div className="toast error">{error}</div>}
      </div>
    </div>
  );
}
