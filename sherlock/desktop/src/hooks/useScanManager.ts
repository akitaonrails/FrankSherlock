import { useState, useRef, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  appHealth,
  cancelScan,
  ensureDatabase,
  getRuntimeStatus,
  getScanJob,
  getSetupStatus,
  listActiveScans,
  listRoots,
  startScan,
  startSetupDownload,
  startVenvProvision,
} from "../api";
import type {
  DbStats,
  RootInfo,
  RuntimeStatus,
  ScanJobStatus,
  SetupStatus,
} from "../types";
import { basename, errorMessage } from "../utils";

/** Shallow-compare two scan arrays to avoid unnecessary re-renders. */
function scansChanged(prev: ScanJobStatus[], next: ScanJobStatus[]): boolean {
  if (prev.length !== next.length) return true;
  for (let i = 0; i < prev.length; i++) {
    if (
      prev[i].id !== next[i].id ||
      prev[i].status !== next[i].status ||
      prev[i].processedFiles !== next[i].processedFiles ||
      prev[i].totalFiles !== next[i].totalFiles ||
      prev[i].added !== next[i].added ||
      prev[i].modified !== next[i].modified ||
      prev[i].moved !== next[i].moved
    ) return true;
  }
  return false;
}

const PAGE_SIZE = 80;
const MAX_ITEMS = 400;

type ScanManagerCallbacks = {
  setSetup: (s: SetupStatus) => void;
  setRuntime: (r: RuntimeStatus) => void;
  setDbStats: (d: DbStats) => void;
  setRoots: (r: RootInfo[]) => void;
  setReadOnly: (ro: boolean) => void;
  setShowResumeModal: (show: boolean) => void;
  setNotice: (msg: string) => void;
  setError: (msg: string) => void;
  runSearch: (offset: number, append: boolean, limitOverride?: number, preserveSelection?: boolean) => Promise<void>;
  itemsLength: () => number;
};

export function useScanManager(cb: ScanManagerCallbacks) {
  const [activeScans, setActiveScans] = useState<ScanJobStatus[]>([]);
  const [trackedJobIds, setTrackedJobIds] = useState<number[]>([]);
  const [completedJobs, setCompletedJobs] = useState<ScanJobStatus[]>([]);
  const lastProcessedRef = useRef(0);

  const refreshRoots = useCallback(async () => {
    try {
      const r = await listRoots();
      cb.setRoots(r);
    } catch {
      // Silently ignore
    }
  }, [cb.setRoots]);

  const pollRuntimeAndScans = useCallback(async () => {
    try {
      const ids = trackedJobIds;
      const [setupStatus, runtimeStatus, scans, ...trackedResults] = await Promise.all([
        getSetupStatus(),
        getRuntimeStatus(),
        listActiveScans(),
        ...ids.map((id) => getScanJob(id).catch(() => null)),
      ]);
      cb.setSetup(setupStatus);
      cb.setRuntime(runtimeStatus);
      setActiveScans((prev) => scansChanged(prev, scans) ? scans : prev);

      if (ids.length === 0) return;

      const trackedJobs = trackedResults.filter((j): j is ScanJobStatus => j !== null);
      const stillTracked: number[] = [];
      const justCompleted: ScanJobStatus[] = [];
      let maxProcessed = 0;
      let anyRunning = false;

      for (const job of trackedJobs) {
        if (job.status === "running" || job.status === "pending" || job.status === "interrupted") {
          stillTracked.push(job.id);
          if (job.status === "running") {
            anyRunning = true;
            if (job.processedFiles > maxProcessed) maxProcessed = job.processedFiles;
          }
        } else if (job.status === "completed") {
          justCompleted.push(job);
        } else if (job.status === "failed") {
          cb.setError(job.errorText || `Scan failed for ${basename(job.rootPath)}`);
        }
      }

      if (anyRunning && maxProcessed > lastProcessedRef.current) {
        lastProcessedRef.current = maxProcessed;
        const liveLimit = Math.max(PAGE_SIZE, Math.min(cb.itemsLength(), MAX_ITEMS));
        void cb.runSearch(0, false, liveLimit, true);
        void refreshRoots();
      }

      if (justCompleted.length > 0) {
        lastProcessedRef.current = 0;
        setCompletedJobs((prev) => [...prev, ...justCompleted]);
        const stats = await ensureDatabase();
        cb.setDbStats(stats);
        await refreshRoots();
        await cb.runSearch(0, false);
      }

      setTrackedJobIds(stillTracked);
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }, [trackedJobIds, cb, refreshRoots]);

  type InitAppResult = {
    roots: RootInfo[];
    scans: ScanJobStatus[];
    setupStatus: SetupStatus;
    readOnly: boolean;
  };

  async function initApp(): Promise<InitAppResult | null> {
    try {
      const [db, setupStatus, runtimeStatus, scans, rootList, health] = await Promise.all([
        ensureDatabase(),
        getSetupStatus(),
        getRuntimeStatus(),
        listActiveScans(),
        listRoots(),
        appHealth(),
      ]);
      cb.setDbStats(db);
      cb.setSetup(setupStatus);
      cb.setRuntime(runtimeStatus);
      setActiveScans(scans);
      cb.setRoots(rootList);
      cb.setReadOnly(health.readOnly);
      const runningIds = scans.filter((s) => s.status === "running").map((s) => s.id);
      if (runningIds.length > 0) {
        setTrackedJobIds(runningIds);
      }
      const interrupted = scans.filter((s) => s.status === "interrupted");
      if (interrupted.length > 0) {
        cb.setShowResumeModal(true);
      }
      return { roots: rootList, scans, setupStatus, readOnly: health.readOnly };
    } catch (err) {
      cb.setError(errorMessage(err));
      return null;
    }
  }

  async function onPickAndScan(setup: SetupStatus | null, readOnly: boolean) {
    if (readOnly) return;
    if (setup && !setup.isReady) {
      cb.setError("Setup is incomplete. Finish Ollama setup before starting scans.");
      return;
    }
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select folder to scan" });
      if (!selected) return;
      setCompletedJobs([]);
      const job = await startScan(selected as string);
      setTrackedJobIds((prev) => [...prev, job.id]);
      lastProcessedRef.current = 0;
      cb.setNotice(`Scan started for ${basename(job.rootPath)}`);
      await refreshRoots();
      await pollRuntimeAndScans();
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }

  async function onRescanRoot(root: RootInfo, setup: SetupStatus | null, readOnly: boolean) {
    if (readOnly) return;
    if (setup && !setup.isReady) {
      cb.setError("Setup is incomplete. Finish Ollama setup before starting scans.");
      return;
    }
    try {
      setCompletedJobs([]);
      const job = await startScan(root.rootPath);
      setTrackedJobIds((prev) => [...prev, job.id]);
      lastProcessedRef.current = 0;
      cb.setNotice(`Rescan started for ${root.rootName}`);
      await refreshRoots();
      await pollRuntimeAndScans();
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }

  async function onCancelScan(scan: ScanJobStatus, readOnly: boolean) {
    if (readOnly) return;
    try {
      await cancelScan(scan.id);
      cb.setNotice(`Cancelling scan for ${basename(scan.rootPath)}...`);
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }

  async function onResumeScan(scan: ScanJobStatus, readOnly: boolean) {
    if (readOnly) return;
    try {
      const job = await startScan(scan.rootPath);
      setTrackedJobIds((prev) => [...prev, job.id]);
      setCompletedJobs([]);
      lastProcessedRef.current = 0;
      cb.setNotice(`Resuming scan for ${basename(job.rootPath)}`);
      await pollRuntimeAndScans();
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }

  async function onResumeAllInterrupted() {
    cb.setShowResumeModal(false);
    const newIds: number[] = [];
    for (const scan of activeScans.filter((s) => s.status === "interrupted")) {
      try {
        const job = await startScan(scan.rootPath);
        newIds.push(job.id);
        lastProcessedRef.current = 0;
      } catch (err) {
        cb.setError(errorMessage(err));
      }
    }
    if (newIds.length > 0) {
      setTrackedJobIds((prev) => [...prev, ...newIds]);
      setCompletedJobs([]);
    }
    await pollRuntimeAndScans();
  }

  async function onSetupDownload() {
    try {
      await startSetupDownload();
      await pollRuntimeAndScans();
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }

  async function onSetupOcr() {
    try {
      await startVenvProvision();
      await pollRuntimeAndScans();
    } catch (err) {
      cb.setError(errorMessage(err));
    }
  }

  async function onRecheckSetup() {
    await pollRuntimeAndScans();
  }

  function addTrackedJobId(id: number) {
    setTrackedJobIds((prev) => [...prev, id]);
  }

  return {
    activeScans,
    trackedJobIds,
    completedJobs,
    setCompletedJobs,
    pollRuntimeAndScans,
    initApp,
    onPickAndScan,
    onRescanRoot,
    onCancelScan,
    onResumeScan,
    onResumeAllInterrupted,
    onSetupDownload,
    onSetupOcr,
    onRecheckSetup,
    refreshRoots,
    addTrackedJobId,
  };
}
