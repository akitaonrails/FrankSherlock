import { invoke } from "@tauri-apps/api/core";
import type {
  AppPaths,
  CleanupResult,
  DbStats,
  HealthStatus,
  PurgeResult,
  RootInfo,
  RuntimeStatus,
  ScanJobStatus,
  SetupDownloadStatus,
  SetupStatus,
  SearchRequest,
  SearchResponse
} from "./types";

export async function appHealth(): Promise<HealthStatus> {
  return invoke<HealthStatus>("app_health");
}

export async function ensureDatabase(): Promise<DbStats> {
  return invoke<DbStats>("ensure_database");
}

export async function getPaths(): Promise<AppPaths> {
  return invoke<AppPaths>("get_app_paths");
}

export async function searchImages(request: SearchRequest): Promise<SearchResponse> {
  return invoke<SearchResponse>("search_images", { request });
}

export async function startScan(rootPath: string): Promise<ScanJobStatus> {
  return invoke<ScanJobStatus>("start_scan", { rootPath });
}

export async function getScanJob(jobId: number): Promise<ScanJobStatus | null> {
  return invoke<ScanJobStatus | null>("get_scan_job", { jobId });
}

export async function listActiveScans(): Promise<ScanJobStatus[]> {
  return invoke<ScanJobStatus[]>("list_active_scans");
}

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  return invoke<RuntimeStatus>("get_runtime_status");
}

export async function getSetupStatus(): Promise<SetupStatus> {
  return invoke<SetupStatus>("get_setup_status");
}

export async function startSetupDownload(): Promise<SetupDownloadStatus> {
  return invoke<SetupDownloadStatus>("start_setup_download");
}

export async function cleanupOllamaModels(): Promise<CleanupResult> {
  return invoke<CleanupResult>("cleanup_ollama_models");
}

export async function cancelScan(jobId: number): Promise<boolean> {
  return invoke<boolean>("cancel_scan", { jobId });
}

export async function removeRoot(rootId: number): Promise<PurgeResult> {
  return invoke<PurgeResult>("remove_root", { rootId });
}

export async function listRoots(): Promise<RootInfo[]> {
  return invoke<RootInfo[]>("list_roots");
}

export async function loadUserConfig(): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("load_user_config");
}

export async function saveUserConfig(config: Record<string, unknown>): Promise<void> {
  return invoke<void>("save_user_config", { config });
}
