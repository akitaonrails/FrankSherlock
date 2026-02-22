import { invoke } from "@tauri-apps/api/core";
import type {
  AppPaths,
  CleanupResult,
  DbStats,
  ScanSummary,
  SearchRequest,
  SearchResponse
} from "./types";

export async function ensureDatabase(): Promise<DbStats> {
  return invoke<DbStats>("ensure_database");
}

export async function getPaths(): Promise<AppPaths> {
  return invoke<AppPaths>("get_app_paths");
}

export async function searchImages(request: SearchRequest): Promise<SearchResponse> {
  return invoke<SearchResponse>("search_images", { request });
}

export async function scanRoot(rootPath: string): Promise<ScanSummary> {
  return invoke<ScanSummary>("scan_root", { rootPath });
}

export async function cleanupOllamaModels(): Promise<CleanupResult> {
  return invoke<CleanupResult>("cleanup_ollama_models");
}
