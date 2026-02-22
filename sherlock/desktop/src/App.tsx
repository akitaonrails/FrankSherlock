import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  cleanupOllamaModels,
  ensureDatabase,
  getPaths,
  getRuntimeStatus,
  getScanJob,
  getSetupStatus,
  listActiveScans,
  searchImages,
  startScan,
  startSetupDownload
} from "./api";
import type {
  AppPaths,
  DbStats,
  RuntimeStatus,
  ScanJobStatus,
  SearchItem,
  SearchResponse,
  SetupStatus
} from "./types";

const PAGE_SIZE = 40;
const POLL_MS = 1200;

export default function App() {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<SearchItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [scanInput, setScanInput] = useState("");
  const [minConfidence, setMinConfidence] = useState(0);
  const [selectedMediaType, setSelectedMediaType] = useState("");
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [activeScans, setActiveScans] = useState<ScanJobStatus[]>([]);
  const [trackedJobId, setTrackedJobId] = useState<number | null>(null);
  const [latestJob, setLatestJob] = useState<ScanJobStatus | null>(null);
  const [previewItem, setPreviewItem] = useState<SearchItem | null>(null);
  const requestIdRef = useRef(0);

  const canLoadMore = items.length < total;
  const mediaTypeOptions = useMemo(
    () => ["", "document", "anime", "screenshot", "photo", "artwork", "manga", "other"],
    []
  );

  const currentScan =
    (trackedJobId ? activeScans.find((job) => job.id === trackedJobId) : null) ??
    activeScans[0] ??
    latestJob;

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const [db, p, setupStatus, runtimeStatus, scans] = await Promise.all([
          ensureDatabase(),
          getPaths(),
          getSetupStatus(),
          getRuntimeStatus(),
          listActiveScans()
        ]);
        if (!mounted) return;
        setDbStats(db);
        setPaths(p);
        setSetup(setupStatus);
        setRuntime(runtimeStatus);
        setActiveScans(scans);
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
    if (setup && !setup.isReady) {
      return;
    }
    const timer = setTimeout(() => {
      void runSearch(0, false);
    }, 260);
    return () => clearTimeout(timer);
  }, [query, minConfidence, selectedMediaType, setup?.isReady]);

  // Keyboard: Esc closes preview
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape" && previewItem) {
        setPreviewItem(null);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [previewItem]);

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
        if (trackedJob.status === "completed") {
          setNotice(
            `Scan completed for ${trackedJob.rootPath}: ${trackedJob.processedFiles}/${trackedJob.totalFiles} files.`
          );
          setTrackedJobId(null);
          const stats = await ensureDatabase();
          setDbStats(stats);
          await runSearch(0, false);
        } else if (trackedJob.status === "failed") {
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
    if (append) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    try {
      const response = await searchImages({
        query,
        limit: PAGE_SIZE,
        offset,
        minConfidence: minConfidence > 0 ? minConfidence : undefined,
        mediaTypes: selectedMediaType ? [selectedMediaType] : undefined
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

  async function onScanSubmit(event: FormEvent) {
    event.preventDefault();
    if (!scanInput.trim()) return;
    if (setup && !setup.isReady) {
      setError("Setup is incomplete. Finish Ollama setup before starting scans.");
      return;
    }
    setError(null);
    setNotice(null);
    try {
      const job = await startScan(scanInput.trim());
      setTrackedJobId(job.id);
      setLatestJob(job);
      setNotice(`Scan started for ${job.rootPath}`);
      await pollRuntimeAndScans();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onCleanupOllama() {
    try {
      const result = await cleanupOllamaModels();
      setNotice(
        `Ollama cleanup: stopped ${result.stoppedModels}/${result.runningModels} loaded model(s).`
      );
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
      return convertFileSrc(item.thumbnailPath);
    }
    return null;
  }

  function openPreview(item: SearchItem) {
    setPreviewItem(item);
  }

  function closePreview() {
    setPreviewItem(null);
  }

  const scanProgress = currentScan?.totalFiles
    ? Math.min(
        100,
        (currentScan.processedFiles / Math.max(1, currentScan.totalFiles)) * 100
      )
    : 0;

  return (
    <main className="layout">
      {setup && !setup.isReady && (
        <div className="modal-overlay" role="dialog" aria-modal="true">
          <div className="setup-modal">
            <h2>First-Time Setup Required</h2>
            <p>Sherlock needs local Ollama service and required model(s) before scanning.</p>
            <div className="setup-status-grid">
              <div>
                <strong>Ollama</strong>
                <p>{setup.ollamaAvailable ? "Running" : "Not detected"}</p>
              </div>
              <div>
                <strong>Required models</strong>
                <p>{setup.requiredModels.join(", ")}</p>
              </div>
              <div>
                <strong>Missing models</strong>
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
              <button type="button" onClick={onRecheckSetup}>
                Recheck setup
              </button>
              <button
                type="button"
                onClick={onSetupDownload}
                disabled={
                  !setup.ollamaAvailable ||
                  setup.missingModels.length === 0 ||
                  setup.download.status === "running"
                }
              >
                {setup.download.status === "running" ? "Downloading..." : "Download required model"}
              </button>
            </div>
          </div>
        </div>
      )}

      {previewItem && (
        <div className="modal-overlay preview-overlay" onClick={closePreview} role="dialog" aria-modal="true">
          <div className="preview-modal" onClick={(e) => e.stopPropagation()}>
            <button className="preview-close" onClick={closePreview} type="button" aria-label="Close preview">
              &times;
            </button>
            <div className="preview-image-wrap">
              <img
                src={convertFileSrc(previewItem.absPath)}
                alt={previewItem.relPath}
              />
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

      <header className="header">
        <div>
          <h1>Sherlock</h1>
          <p>Local-only image intelligence with global search index.</p>
        </div>
        <button className="ghost" onClick={onCleanupOllama} type="button">
          Cleanup Ollama
        </button>
      </header>

      <section className="panel">
        <div className="meta">
          <span>Indexed files: {dbStats?.files ?? "..."}</span>
          <span>Roots: {dbStats?.roots ?? "..."}</span>
          <span>DB: {paths?.dbFile ?? "..."}</span>
          <span>Model: {runtime?.currentModel || "none"}</span>
          <span>
            VRAM:{" "}
            {runtime?.vramUsedMib != null && runtime?.vramTotalMib != null
              ? `${runtime.vramUsedMib} / ${runtime.vramTotalMib} MiB`
              : "n/a"}
          </span>
        </div>
        <form className="scan-form" onSubmit={onScanSubmit}>
          <input
            type="text"
            value={scanInput}
            onChange={(e) => setScanInput(e.target.value)}
            placeholder="/mnt/terachad/Dropbox"
            aria-label="Scan root path"
            disabled={setup ? !setup.isReady : true}
          />
          <button type="submit" disabled={setup ? !setup.isReady : true}>
            Scan Root
          </button>
        </form>
        {currentScan && (
          <div className="scan-progress">
            <div className="scan-progress-head">
              <strong>
                Scan {currentScan.status}: {currentScan.processedFiles} / {currentScan.totalFiles}
              </strong>
              <span>{scanProgress.toFixed(1)}%</span>
            </div>
            <progress value={scanProgress} max={100} />
            <p className="scan-summary">
              added {currentScan.added}, modified {currentScan.modified}, moved {currentScan.moved},
              unchanged {currentScan.unchanged}, deleted {currentScan.deleted}
            </p>
          </div>
        )}
      </section>

      <section className="panel">
        <div className="search-controls">
          <input
            type="search"
            placeholder="Query naturally: 'anime girl from 2024 confidence >= 0.8'"
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
                {opt ? opt : "all media types"}
              </option>
            ))}
          </select>
          <label className="confidence">
            Min confidence
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

        <div className="results-meta">
          <span>
            Showing {items.length} of {total} results
          </span>
          {loading && <span>Searching...</span>}
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
                onClick={() => openPreview(item)}
                onKeyDown={(e) => {
                  if (e.key === " " || e.key === "Enter") {
                    e.preventDefault();
                    openPreview(item);
                  }
                }}
              >
                <div className="thumb">
                  {thumb ? (
                    <img src={thumb} alt={item.relPath} loading="lazy" />
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
      </section>

      {notice && <p className="notice">{notice}</p>}
      {error && <p className="error">{error}</p>}
    </main>
  );
}
