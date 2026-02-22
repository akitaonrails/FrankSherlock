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

export type ScanSummary = {
  rootId: number;
  rootPath: string;
  scanned: number;
  added: number;
  modified: number;
  moved: number;
  unchanged: number;
  deleted: number;
  elapsedMs: number;
};

export type DbStats = {
  roots: number;
  files: number;
};

export type CleanupResult = {
  runningModels: number;
  stoppedModels: number;
};

export type AppPaths = {
  baseDir: string;
  dbFile: string;
  cacheDir: string;
};
