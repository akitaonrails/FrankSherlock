import type { Album, DuplicateFile, DuplicateGroup, DuplicatesResponse, RootInfo, ScanJobStatus, SearchItem, SmartFolder } from "../types";

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

export const mockAlbum: Album = {
  id: 1,
  name: "Vacation",
  createdAt: 0,
  fileCount: 5,
};

export const mockSmartFolder: SmartFolder = {
  id: 1,
  name: "Anime photos",
  query: "anime photo",
  createdAt: 0,
};

export const mockDuplicateFileKeeper: DuplicateFile = {
  id: 10,
  rootId: 1,
  relPath: "photos/original.jpg",
  absPath: "/home/user/photos/original.jpg",
  rootPath: "/home/user/photos",
  mediaType: "photo",
  description: "A sunset photo",
  confidence: 0.9,
  mtimeNs: 1000000000000,
  sizeBytes: 5000,
  thumbnailPath: "/cache/thumb10.jpg",
  isKeeper: true,
  groupType: "exact",
};

export const mockDuplicateFileCopy: DuplicateFile = {
  id: 11,
  rootId: 1,
  relPath: "backup/original_copy.jpg",
  absPath: "/home/user/photos/backup/original_copy.jpg",
  rootPath: "/home/user/photos",
  mediaType: "photo",
  description: "A sunset photo",
  confidence: 0.9,
  mtimeNs: 2000000000000,
  sizeBytes: 5000,
  thumbnailPath: "/cache/thumb11.jpg",
  isKeeper: false,
  groupType: "exact",
};

export const mockDuplicateGroup: DuplicateGroup = {
  fingerprint: "abc123",
  fileCount: 2,
  totalSizeBytes: 10000,
  wastedBytes: 5000,
  files: [mockDuplicateFileKeeper, mockDuplicateFileCopy],
  groupType: "exact",
  avgSimilarity: null,
};

export const mockDuplicatesResponse: DuplicatesResponse = {
  totalGroups: 1,
  totalDuplicateFiles: 1,
  totalWastedBytes: 5000,
  groups: [mockDuplicateGroup],
};
