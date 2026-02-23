import type { Album, DbStats, RootInfo, ScanJobStatus, SmartFolder } from "../../types";
import { formatBytes } from "../../utils/format";
import RootCard from "./RootCard";
import AlbumCard from "./AlbumCard";
import SmartFolderCard from "./SmartFolderCard";
import ScanProgress from "./ScanProgress";
import "./Sidebar.css";

type SidebarProps = {
  roots: RootInfo[];
  selectedRootId: number | null;
  activeScans: ScanJobStatus[];
  dbStats: DbStats | null;
  readOnly: boolean;
  setupReady: boolean;
  albums: Album[];
  smartFolders: SmartFolder[];
  activeAlbumName: string | null;
  activeSmartFolderId: number | null;
  onSelectRoot: (rootId: number | null) => void;
  onDeleteRoot: (root: RootInfo) => void;
  onPickAndScan: () => void;
  onRescanRoot: (root: RootInfo) => void;
  onCancelScan: (scan: ScanJobStatus) => void;
  onResumeScan: (scan: ScanJobStatus) => void;
  onSelectAlbum: (album: Album) => void;
  onDeleteAlbum: (album: Album) => void;
  onSelectSmartFolder: (folder: SmartFolder) => void;
  onDeleteSmartFolder: (folder: SmartFolder) => void;
};

export default function Sidebar({
  roots, selectedRootId, activeScans, dbStats, readOnly,
  setupReady, albums, smartFolders, activeAlbumName, activeSmartFolderId,
  onSelectRoot, onDeleteRoot, onRescanRoot, onPickAndScan,
  onCancelScan, onResumeScan,
  onSelectAlbum, onDeleteAlbum, onSelectSmartFolder, onDeleteSmartFolder,
}: SidebarProps) {
  const runningScans = activeScans.filter((s) => s.status === "running");
  const interruptedScans = activeScans.filter((s) => s.status === "interrupted");

  function scanForRoot(rootId: number): ScanJobStatus | undefined {
    return activeScans.find((s) => s.rootId === rootId && s.status === "running");
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-scroll">
        <div className="sidebar-section">
          <span>Folders</span>
          {!readOnly && (
            <button
              type="button"
              className="sidebar-add-btn"
              onClick={onPickAndScan}
              disabled={!setupReady}
              title="Add folder to scan"
            >+</button>
          )}
        </div>

        {roots.length === 0 && (
          <div className="sidebar-empty">No folders scanned yet</div>
        )}

        <div className="root-list">
          {roots.map((root) => (
            <RootCard
              key={root.id}
              root={root}
              isSelected={selectedRootId === root.id}
              scan={scanForRoot(root.id)}
              readOnly={readOnly}
              onSelect={() => onSelectRoot(selectedRootId === root.id ? null : root.id)}
              onDelete={() => onDeleteRoot(root)}
              onRescan={() => onRescanRoot(root)}
            />
          ))}
        </div>

        {runningScans.map((scan) => (
          <ScanProgress
            key={scan.id}
            scan={scan}
            readOnly={readOnly}
            onCancel={() => onCancelScan(scan)}
          />
        ))}
        {interruptedScans.map((scan) => (
          <ScanProgress
            key={scan.id}
            scan={scan}
            readOnly={readOnly}
            onResume={() => onResumeScan(scan)}
          />
        ))}

        {albums.length > 0 && (
          <>
            <div className="sidebar-section"><span>Albums</span></div>
            <div className="root-list">
              {albums.map((album) => (
                <AlbumCard
                  key={album.id}
                  album={album}
                  isSelected={activeAlbumName?.toLowerCase() === album.name.toLowerCase()}
                  onSelect={() => onSelectAlbum(album)}
                  onDelete={() => onDeleteAlbum(album)}
                />
              ))}
            </div>
          </>
        )}

        {smartFolders.length > 0 && (
          <>
            <div className="sidebar-section"><span>Smart Folders</span></div>
            <div className="root-list">
              {smartFolders.map((folder) => (
                <SmartFolderCard
                  key={folder.id}
                  folder={folder}
                  isSelected={activeSmartFolderId === folder.id}
                  onSelect={() => onSelectSmartFolder(folder)}
                  onDelete={() => onDeleteSmartFolder(folder)}
                />
              ))}
            </div>
          </>
        )}
      </div>

      <div className="sidebar-info-fixed">
        <div className="sidebar-section"><span>Info</span></div>
        <div className="sidebar-item">Files: <span>{dbStats?.files ?? "..."}</span></div>
        <div className="sidebar-item">DB size: <span>{dbStats ? formatBytes(dbStats.dbSizeBytes) : "..."}</span></div>
        <div className="sidebar-item">Thumbs: <span>{dbStats ? formatBytes(dbStats.thumbsSizeBytes) : "..."}</span></div>
      </div>
    </aside>
  );
}
