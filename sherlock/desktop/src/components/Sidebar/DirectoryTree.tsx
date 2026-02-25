import { useCallback, useEffect, useState } from "react";
import { listSubdirectories } from "../../api";
import type { SubdirEntry } from "../../types";

type DirectoryTreeProps = {
  rootId: number;
  selectedSubdir: string | null;
  onSelectSubdir: (subdir: string | null) => void;
};

type TreeNodeProps = {
  entry: SubdirEntry;
  level: number;
  selectedSubdir: string | null;
  expandedNodes: Set<string>;
  childrenCache: Map<string, SubdirEntry[]>;
  onToggleExpand: (relPath: string) => void;
  onSelect: (relPath: string) => void;
};

function TreeNode({
  entry, level, selectedSubdir, expandedNodes, childrenCache,
  onToggleExpand, onSelect,
}: TreeNodeProps) {
  const isExpanded = expandedNodes.has(entry.relPath);
  const isSelected = selectedSubdir === entry.relPath;
  const children = childrenCache.get(entry.relPath);

  return (
    <>
      <div
        className={`dir-tree-node${isSelected ? " dir-tree-selected" : ""}`}
        style={{ paddingLeft: `${8 + level * 16}px` }}
      >
        <button
          type="button"
          className="dir-tree-chevron"
          onClick={(e) => { e.stopPropagation(); onToggleExpand(entry.relPath); }}
          aria-label={isExpanded ? `Collapse ${entry.name}` : `Expand ${entry.name}`}
        >
          {isExpanded ? "\u25BE" : "\u25B8"}
        </button>
        <button
          type="button"
          className="dir-tree-label"
          onClick={() => onSelect(entry.relPath)}
          title={entry.relPath}
        >
          <span className="dir-tree-name">{entry.name}</span>
          <span className="dir-tree-count">{entry.fileCount}</span>
        </button>
      </div>
      {isExpanded && children && children.map((child) => (
        <TreeNode
          key={child.relPath}
          entry={child}
          level={level + 1}
          selectedSubdir={selectedSubdir}
          expandedNodes={expandedNodes}
          childrenCache={childrenCache}
          onToggleExpand={onToggleExpand}
          onSelect={onSelect}
        />
      ))}
    </>
  );
}

export default function DirectoryTree({ rootId, selectedSubdir, onSelectSubdir }: DirectoryTreeProps) {
  const [topLevel, setTopLevel] = useState<SubdirEntry[]>([]);
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(new Set());
  const [childrenCache, setChildrenCache] = useState<Map<string, SubdirEntry[]>>(new Map());
  const [loading, setLoading] = useState(false);

  // Fetch top-level directories when rootId changes
  useEffect(() => {
    let cancelled = false;
    setTopLevel([]);
    setExpandedNodes(new Set());
    setChildrenCache(new Map());
    setLoading(true);

    listSubdirectories(rootId, "")
      .then((dirs) => {
        if (!cancelled) setTopLevel(dirs);
      })
      .catch(() => {
        if (!cancelled) setTopLevel([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [rootId]);

  const onToggleExpand = useCallback(async (relPath: string) => {
    setExpandedNodes((prev) => {
      const next = new Set(prev);
      if (next.has(relPath)) {
        next.delete(relPath);
      } else {
        next.add(relPath);
        // Lazy-fetch children if not cached
        if (!childrenCache.has(relPath)) {
          listSubdirectories(rootId, relPath)
            .then((dirs) => {
              setChildrenCache((c) => new Map(c).set(relPath, dirs));
            })
            .catch(() => {
              setChildrenCache((c) => new Map(c).set(relPath, []));
            });
        }
      }
      return next;
    });
  }, [rootId, childrenCache]);

  const onSelect = useCallback((relPath: string) => {
    onSelectSubdir(selectedSubdir === relPath ? null : relPath);
  }, [selectedSubdir, onSelectSubdir]);

  if (loading) {
    return <div className="dir-tree-loading">Loading...</div>;
  }

  if (topLevel.length === 0) {
    return null;
  }

  return (
    <div className="dir-tree" role="tree" aria-label="Directory tree">
      {topLevel.map((entry) => (
        <TreeNode
          key={entry.relPath}
          entry={entry}
          level={0}
          selectedSubdir={selectedSubdir}
          expandedNodes={expandedNodes}
          childrenCache={childrenCache}
          onToggleExpand={onToggleExpand}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}
