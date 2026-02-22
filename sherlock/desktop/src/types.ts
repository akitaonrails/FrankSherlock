export type SearchItem = {
  id: number;
  rootId: number;
  relPath: string;
  absPath: string;
  mediaType: string;
  description: string;
  confidence: number;
  mtimeNs: number;
  sizeBytes: number;
  thumbnailPath?: string | null;
};

export type ParsedQuery = {
  rawQuery: string;
  queryText: string;
  mediaTypes: string[];
  dateFrom?: string | null;
  dateTo?: string | null;
  minConfidence?: number | null;
  rootHints: string[];
  parserConfidence: number;
};

export type SearchResponse = {
  total: number;
  limit: number;
  offset: number;
  items: SearchItem[];
  parsedQuery: ParsedQuery;
};

export type SearchRequest = {
  query: string;
  limit?: number;
  offset?: number;
  rootScope?: number[];
  mediaTypes?: string[];
  minConfidence?: number;
  dateFrom?: string;
  dateTo?: string;
};

export type ScanJobStatus = {
  id: number;
  rootId: number;
  rootPath: string;
  status: "pending" | "running" | "interrupted" | "completed" | "failed";
  scanMarker: number;
  totalFiles: number;
  processedFiles: number;
  progressPct: number;
  added: number;
  modified: number;
  moved: number;
  unchanged: number;
  deleted: number;
  cursorRelPath?: string | null;
  errorText?: string | null;
  updatedAt: number;
  startedAt: number;
  completedAt?: number | null;
};

export type DbStats = {
  roots: number;
  files: number;
};

export type CleanupResult = {
  runningModels: number;
  stoppedModels: number;
};

export type RuntimeStatus = {
  currentModel?: string | null;
  loadedModels: string[];
  vramUsedMib?: number | null;
  vramTotalMib?: number | null;
  ollamaAvailable: boolean;
};

export type SetupDownloadStatus = {
  status: "idle" | "running" | "completed" | "failed";
  model?: string | null;
  progressPct: number;
  message: string;
};

export type SetupStatus = {
  isReady: boolean;
  ollamaAvailable: boolean;
  requiredModels: string[];
  missingModels: string[];
  instructions: string[];
  download: SetupDownloadStatus;
};

export type AppPaths = {
  baseDir: string;
  dbFile: string;
  cacheDir: string;
};
