import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import DirectoryTree from "../../components/Sidebar/DirectoryTree";
import type { SubdirEntry } from "../../types";

// Mock the API
vi.mock("../../api", () => ({
  listSubdirectories: vi.fn(),
}));

import { listSubdirectories } from "../../api";
const mockListSubdirs = vi.mocked(listSubdirectories);

const topLevelDirs: SubdirEntry[] = [
  { relPath: "Documents", name: "Documents", fileCount: 5 },
  { relPath: "Photos", name: "Photos", fileCount: 20 },
];

const photosChildren: SubdirEntry[] = [
  { relPath: "Photos/2023", name: "2023", fileCount: 8 },
  { relPath: "Photos/2024", name: "2024", fileCount: 12 },
];

beforeEach(() => {
  vi.clearAllMocks();
});

describe("DirectoryTree", () => {
  it("renders top-level directories on mount", async () => {
    mockListSubdirs.mockResolvedValueOnce(topLevelDirs);

    render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Documents")).toBeInTheDocument();
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    expect(mockListSubdirs).toHaveBeenCalledWith(1, "");
  });

  it("renders nothing when no subdirectories", async () => {
    mockListSubdirs.mockResolvedValueOnce([]);

    const { container } = render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.queryByText("Loading...")).not.toBeInTheDocument();
    });

    expect(container.querySelector(".dir-tree")).toBeNull();
  });

  it("calls onSelectSubdir when directory name is clicked", async () => {
    mockListSubdirs.mockResolvedValueOnce(topLevelDirs);
    const onSelectSubdir = vi.fn();

    render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={onSelectSubdir} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Photos"));
    expect(onSelectSubdir).toHaveBeenCalledWith("Photos");
  });

  it("deselects when clicking already-selected directory", async () => {
    mockListSubdirs.mockResolvedValueOnce(topLevelDirs);
    const onSelectSubdir = vi.fn();

    render(
      <DirectoryTree rootId={1} selectedSubdir="Photos" onSelectSubdir={onSelectSubdir} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Photos"));
    expect(onSelectSubdir).toHaveBeenCalledWith(null);
  });

  it("expands node and fetches children on chevron click", async () => {
    mockListSubdirs
      .mockResolvedValueOnce(topLevelDirs) // initial load
      .mockResolvedValueOnce(photosChildren); // expand Photos

    render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    const expandBtn = screen.getByLabelText("Expand Photos");
    await userEvent.click(expandBtn);

    await waitFor(() => {
      expect(screen.getByText("2023")).toBeInTheDocument();
      expect(screen.getByText("2024")).toBeInTheDocument();
    });

    expect(mockListSubdirs).toHaveBeenCalledWith(1, "Photos");
  });

  it("collapses expanded node on second chevron click", async () => {
    mockListSubdirs
      .mockResolvedValueOnce(topLevelDirs)
      .mockResolvedValueOnce(photosChildren);

    render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    // Expand
    await userEvent.click(screen.getByLabelText("Expand Photos"));
    await waitFor(() => {
      expect(screen.getByText("2023")).toBeInTheDocument();
    });

    // Collapse
    await userEvent.click(screen.getByLabelText("Collapse Photos"));
    expect(screen.queryByText("2023")).not.toBeInTheDocument();
  });

  it("shows file count badges", async () => {
    mockListSubdirs.mockResolvedValueOnce(topLevelDirs);

    render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("5")).toBeInTheDocument();
      expect(screen.getByText("20")).toBeInTheDocument();
    });
  });

  it("highlights selected directory", async () => {
    mockListSubdirs.mockResolvedValueOnce(topLevelDirs);

    render(
      <DirectoryTree rootId={1} selectedSubdir="Photos" onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    const node = screen.getByText("Photos").closest(".dir-tree-node");
    expect(node?.className).toContain("dir-tree-selected");
  });

  it("resets state when rootId changes", async () => {
    mockListSubdirs
      .mockResolvedValueOnce(topLevelDirs) // root 1
      .mockResolvedValueOnce([{ relPath: "Music", name: "Music", fileCount: 3 }]); // root 2

    const { rerender } = render(
      <DirectoryTree rootId={1} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Photos")).toBeInTheDocument();
    });

    rerender(
      <DirectoryTree rootId={2} selectedSubdir={null} onSelectSubdir={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Music")).toBeInTheDocument();
    });

    expect(screen.queryByText("Photos")).not.toBeInTheDocument();
  });
});
