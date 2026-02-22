import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { cleanupOllamaModels, ensureDatabase, getPaths, scanRoot, searchImages } from "./api";
import type { AppPaths, DbStats, ScanSummary, SearchItem, SearchResponse } from "./types";

const PAGE_SIZE = 80;

export default function App() {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<SearchItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [scanInput, setScanInput] = useState("");
  const [scanSummary, setScanSummary] = useState<ScanSummary | null>(null);
  const [minConfidence, setMinConfidence] = useState(0);
  const [selectedMediaType, setSelectedMediaType] = useState("");
  const requestIdRef = useRef(0);

  const canLoadMore = items.length < total;
  const mediaTypeOptions = useMemo(
    () => ["", "document", "anime", "screenshot", "photo", "artwork", "other"],
    []
  );

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const [db, p] = await Promise.all([ensureDatabase(), getPaths()]);
        if (!mounted) return;
        setDbStats(db);
        setPaths(p);
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
    const timer = setTimeout(() => {
      void runSearch(0, false);
    }, 240);
    return () => clearTimeout(timer);
  }, [query, minConfidence, selectedMediaType]);

  async function runSearch(offset: number, append: boolean) {
    const reqId = ++requestIdRef.current;
    if (append) {
      setLoadingMore(true);
    } else {
      setLoading(true);
      setError(null);
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
    setError(null);
    try {
      const summary = await scanRoot(scanInput.trim());
      setScanSummary(summary);
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
      setError(
        `Ollama cleanup: stopped ${result.stoppedModels}/${result.runningModels} loaded model(s).`
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <main className="layout">
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
        </div>
        <form className="scan-form" onSubmit={onScanSubmit}>
          <input
            type="text"
            value={scanInput}
            onChange={(e) => setScanInput(e.target.value)}
            placeholder="/mnt/terachad/Dropbox"
            aria-label="Scan root path"
          />
          <button type="submit">Scan Root</button>
        </form>
        {scanSummary && (
          <p className="scan-summary">
            Scan done: scanned {scanSummary.scanned}, added {scanSummary.added}, modified{" "}
            {scanSummary.modified}, moved {scanSummary.moved}, deleted {scanSummary.deleted} (
            {scanSummary.elapsedMs} ms)
          </p>
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
            Showing {items.length} / {total}
          </span>
          {loading && <span>Searching...</span>}
        </div>

        <div className="grid" role="list">
          {items.map((item) => (
            <article key={item.id} className="tile" role="listitem">
              <div className="thumb">
                {item.thumbnailPath ? (
                  <img src={`asset://${item.thumbnailPath}`} alt={item.relPath} loading="lazy" />
                ) : (
                  <span>{item.mediaType}</span>
                )}
              </div>
              <div className="tile-body">
                <h3 title={item.relPath}>{item.relPath}</h3>
                <p>{item.description || "No description yet"}</p>
                <div className="tile-meta">
                  <span>{item.mediaType}</span>
                  <span>{item.confidence.toFixed(2)}</span>
                </div>
              </div>
            </article>
          ))}
        </div>

        {canLoadMore && (
          <div className="load-more">
            <button type="button" onClick={onLoadMore} disabled={loadingMore}>
              {loadingMore ? "Loading..." : "Load more"}
            </button>
          </div>
        )}
      </section>

      {error && <p className="error">{error}</p>}
    </main>
  );
}
