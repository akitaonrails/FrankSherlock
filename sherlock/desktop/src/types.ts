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
  parserConfidence: number;
  albumName?: string | null;
};

export type SearchResponse = {
  total: number;
  limit: number;
  offset: number;
  items: SearchItem[];
  parsedQuery: ParsedQuery;
};

export type SortField = "relevance" | "dateModified" | "name" | "type";
export type SortOrder = "asc" | "desc";

export type SearchRequest = {
  query: string;
  limit?: number;
  offset?: number;
  rootScope?: number[];
  mediaTypes?: string[];
  minConfidence?: number;
  dateFrom?: string;
  dateTo?: string;
  sortBy?: SortField;
  sortOrder?: SortOrder;
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
  phase: "discovering" | "processing" | "cleanup";
  discoveredFiles: number;
};

export type DbStats = {
  roots: number;
  files: number;
  dbSizeBytes: number;
  thumbsSizeBytes: number;
};

export type RuntimeStatus = {
  os: "linux" | "macos" | "windows";
  currentModel?: string | null;
  loadedModels: string[];
  vramUsedMib?: number | null;
  vramTotalMib?: number | null;
  gpuVendor: string;
  unifiedMemory: boolean;
  systemRamMib: number;
  ollamaAvailable: boolean;
};

export type SetupDownloadStatus = {
  status: "idle" | "running" | "completed" | "failed";
  model?: string | null;
  progressPct: number;
  message: string;
};

export type VenvProvisionStatus = {
  status: "idle" | "running" | "completed" | "failed";
  step: string;
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
  pythonAvailable: boolean;
  pythonVersion: string | null;
  suryaVenvOk: boolean;
  recommendedModel: string;
  modelTier: string;
  modelSelectionReason: string;
  systemPythonFound: boolean;
  venvProvision: VenvProvisionStatus;
  ffmpegAvailable: boolean;
};

export type HealthStatus = {
  status: string;
  mode: string;
  readOnly: boolean;
};

export type AppPaths = {
  baseDir: string;
  dbFile: string;
  cacheDir: string;
};

export type PurgeResult = {
  filesRemoved: number;
  jobsRemoved: number;
  thumbsCleaned: number;
};

export type RootInfo = {
  id: number;
  rootPath: string;
  rootName: string;
  createdAt: number;
  lastScanAt: number | null;
  fileCount: number;
};

export type DeleteFilesResult = {
  deletedCount: number;
  errors: string[];
};

export type RenameFileResult = {
  fileId: number;
  newRelPath: string;
  newAbsPath: string;
  newFilename: string;
};

export type FileMetadata = {
  id: number;
  mediaType: string;
  description: string;
  extractedText: string;
  canonicalMentions: string;
  locationText: string;
};

export type FileProperties = {
  id: number;
  filename: string;
  absPath: string;
  relPath: string;
  rootPath: string;
  mediaType: string;
  description: string;
  extractedText: string;
  canonicalMentions: string;
  locationText: string;
  confidence: number;
  sizeBytes: number;
  mtimeNs: number;
  fingerprint: string;
  imageWidth?: number | null;
  imageHeight?: number | null;
  cameraMake?: string | null;
  cameraModel?: string | null;
  lensModel?: string | null;
  focalLength?: string | null;
  aperture?: string | null;
  exposureTime?: string | null;
  iso?: string | null;
  dateTaken?: string | null;
  colorSpace?: string | null;
  latitude?: number | null;
  longitude?: number | null;
  gpsLocation?: string | null;
  durationSecs?: number | null;
  videoWidth?: number | null;
  videoHeight?: number | null;
  videoCodec?: string | null;
  audioCodec?: string | null;
};

export type Album = {
  id: number;
  name: string;
  createdAt: number;
  fileCount: number;
};

export type SmartFolder = {
  id: number;
  name: string;
  query: string;
  createdAt: number;
};

export type PdfPassword = {
  id: number;
  password: string;
  label: string;
  createdAt: number;
};

export type ProtectedPdfInfo = {
  id: number;
  filename: string;
  relPath: string;
  absPath: string;
  rootPath: string;
};

export type RetryProtectedPdfsResult = {
  totalAttempted: number;
  unlocked: number;
  stillProtected: number;
};

export type SubdirEntry = {
  relPath: string;
  name: string;
  fileCount: number;
};

export type UpdateInfo = {
  version: string;
  body: string | null;
};

export type DuplicateFile = {
  id: number;
  rootId: number;
  relPath: string;
  absPath: string;
  rootPath: string;
  mediaType: string;
  description: string;
  confidence: number;
  mtimeNs: number;
  sizeBytes: number;
  thumbnailPath?: string | null;
  isKeeper: boolean;
  similarityScore?: number | null;
  groupType: string;
};

export type DuplicateGroup = {
  fingerprint: string;
  fileCount: number;
  totalSizeBytes: number;
  wastedBytes: number;
  files: DuplicateFile[];
  groupType: string;
  avgSimilarity?: number | null;
};

export type DuplicatesResponse = {
  totalGroups: number;
  totalDuplicateFiles: number;
  totalWastedBytes: number;
  groups: DuplicateGroup[];
};
