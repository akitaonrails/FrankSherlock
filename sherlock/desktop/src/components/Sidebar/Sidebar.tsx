import type { Album, DbStats, RootInfo, ScanJobStatus, SmartFolder } from "../../types";
import { formatBytes } from "../../utils/format";
import { useDragReorder } from "../../hooks/useDragReorder";
import RootCard from "./RootCard";
import AlbumCard from "./AlbumCard";
import SmartFolderCard from "./SmartFolderCard";
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
  onCopyRootPath: (root: RootInfo) => void;
  onCancelScan: (scan: ScanJobStatus) => void;
  onResumeScan: (scan: ScanJobStatus) => void;
  onSelectAlbum: (album: Album) => void;
  onDeleteAlbum: (album: Album) => void;
  onSelectSmartFolder: (folder: SmartFolder) => void;
  onDeleteSmartFolder: (folder: SmartFolder) => void;
  onReorderRoots?: (ids: number[]) => void;
  onReorderAlbums?: (ids: number[]) => void;
  onReorderSmartFolders?: (ids: number[]) => void;
  onFindDuplicates?: () => void;
  onOpenPdfPasswords?: () => void;
};

export default function Sidebar({
  roots, selectedRootId, activeScans, dbStats, readOnly,
  setupReady, albums, smartFolders, activeAlbumName, activeSmartFolderId,
  onSelectRoot, onDeleteRoot, onRescanRoot, onCopyRootPath, onPickAndScan,
  onCancelScan, onResumeScan,
  onSelectAlbum, onDeleteAlbum, onSelectSmartFolder, onDeleteSmartFolder,
  onReorderRoots, onReorderAlbums, onReorderSmartFolders, onFindDuplicates,
  onOpenPdfPasswords,
}: SidebarProps) {
  const rootsDrag = useDragReorder({ items: roots, onReorder: onReorderRoots ?? (() => {}), readOnly });
  const albumsDrag = useDragReorder({ items: albums, onReorder: onReorderAlbums ?? (() => {}), readOnly });
  const smartFoldersDrag = useDragReorder({ items: smartFolders, onReorder: onReorderSmartFolders ?? (() => {}), readOnly });

  function scanForRoot(rootId: number): ScanJobStatus | undefined {
    return activeScans.find((s) => s.rootId === rootId && (s.status === "running" || s.status === "interrupted"));
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
          {roots.map((root, i) => {
            const scan = scanForRoot(root.id);
            const dragProps = rootsDrag.getDragProps(i);
            return (
              <div key={root.id} {...dragProps} className={dragProps.className}>
                <RootCard
                  root={root}
                  isSelected={selectedRootId === root.id}
                  scan={scan}
                  readOnly={readOnly}
                  onSelect={() => onSelectRoot(selectedRootId === root.id ? null : root.id)}
                  onDelete={() => onDeleteRoot(root)}
                  onRescan={() => onRescanRoot(root)}
                  onCopyPath={() => onCopyRootPath(root)}
                  onCancelScan={scan?.status === "running" ? () => onCancelScan(scan) : undefined}
                  onResumeScan={scan?.status === "interrupted" ? () => onResumeScan(scan) : undefined}
                />
              </div>
            );
          })}
        </div>

        {albums.length > 0 && (
          <>
            <div className="sidebar-section"><span>Albums</span></div>
            <div className="root-list">
              {albums.map((album, i) => {
                const dragProps = albumsDrag.getDragProps(i);
                return (
                  <div key={album.id} {...dragProps} className={dragProps.className}>
                    <AlbumCard
                      album={album}
                      isSelected={activeAlbumName?.toLowerCase() === album.name.toLowerCase()}
                      onSelect={() => onSelectAlbum(album)}
                      onDelete={() => onDeleteAlbum(album)}
                    />
                  </div>
                );
              })}
            </div>
          </>
        )}

        {smartFolders.length > 0 && (
          <>
            <div className="sidebar-section"><span>Smart Folders</span></div>
            <div className="root-list">
              {smartFolders.map((folder, i) => {
                const dragProps = smartFoldersDrag.getDragProps(i);
                return (
                  <div key={folder.id} {...dragProps} className={dragProps.className}>
                    <SmartFolderCard
                      folder={folder}
                      isSelected={activeSmartFolderId === folder.id}
                      onSelect={() => onSelectSmartFolder(folder)}
                      onDelete={() => onDeleteSmartFolder(folder)}
                    />
                  </div>
                );
              })}
            </div>
          </>
        )}
        {(onFindDuplicates || onOpenPdfPasswords) && (
          <>
            <div className="sidebar-section"><span>Tools</span></div>
            <div className="sidebar-tool-list">
              {onFindDuplicates && (
                <button
                  type="button"
                  className="sidebar-tool-btn"
                  onClick={onFindDuplicates}
                  title="Find duplicate files across all folders"
                >
                  Find Duplicates
                </button>
              )}
              {onOpenPdfPasswords && (
                <button
                  type="button"
                  className="sidebar-tool-btn"
                  onClick={onOpenPdfPasswords}
                  title="Manage passwords for protected PDFs"
                >
                  PDF Passwords
                </button>
              )}
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
