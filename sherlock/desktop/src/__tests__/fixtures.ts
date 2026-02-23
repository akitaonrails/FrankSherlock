import type { RootInfo, ScanJobStatus, SearchItem } from "../types";

export const mockSearchItem: SearchItem = {
  id: 1,
  rootId: 1,
  relPath: "photos/beach.jpg",
  absPath: "/home/user/photos/beach.jpg",
  mediaType: "photo",
  description: "A sunny beach",
  confidence: 0.95,
  mtimeNs: 0,
  sizeBytes: 1024,
  thumbnailPath: "/cache/thumb.jpg",
};

export const mockRoot: RootInfo = {
  id: 1,
  rootPath: "/home/user/photos",
  rootName: "photos",
  createdAt: 0,
  lastScanAt: null,
  fileCount: 42,
};

export const mockRunningScan: ScanJobStatus = {
  id: 10,
  rootId: 1,
  rootPath: "/home/user/photos",
  status: "running",
  scanMarker: 0,
  totalFiles: 100,
  processedFiles: 50,
  progressPct: 50,
  added: 10,
  modified: 5,
  moved: 2,
  unchanged: 33,
  deleted: 0,
  startedAt: 0,
  updatedAt: 0,
};
