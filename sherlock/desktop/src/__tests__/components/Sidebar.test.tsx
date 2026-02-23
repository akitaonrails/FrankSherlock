import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Sidebar from "../../components/Sidebar/Sidebar";
import type { Album, RootInfo, ScanJobStatus, SmartFolder } from "../../types";
import { mockRoot as sampleRoot, mockRunningScan } from "../fixtures";

const defaultProps = {
  roots: [] as RootInfo[],
  selectedRootId: null,
  activeScans: [] as ScanJobStatus[],
  dbStats: null,
  readOnly: false,
  setupReady: true,
  albums: [] as Album[],
  smartFolders: [] as SmartFolder[],
  activeAlbumName: null,
  activeSmartFolderId: null,
  onSelectRoot: vi.fn(),
  onDeleteRoot: vi.fn(),
  onRescanRoot: vi.fn(),
  onPickAndScan: vi.fn(),
  onCancelScan: vi.fn(),
  onResumeScan: vi.fn(),
  onSelectAlbum: vi.fn(),
  onDeleteAlbum: vi.fn(),
  onSelectSmartFolder: vi.fn(),
  onDeleteSmartFolder: vi.fn(),
  onReorderRoots: vi.fn(),
  onReorderAlbums: vi.fn(),
  onReorderSmartFolders: vi.fn(),
};

describe("Sidebar", () => {
  it("shows empty message when no roots", () => {
    render(<Sidebar {...defaultProps} />);
    expect(screen.getByText("No folders scanned yet")).toBeInTheDocument();
  });

  it("renders root cards", () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} />);
    expect(screen.getByText("photos")).toBeInTheDocument();
    expect(screen.getByText("42 files")).toBeInTheDocument();
  });

  it("calls onSelectRoot when root card clicked", async () => {
    const onSelectRoot = vi.fn();
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} onSelectRoot={onSelectRoot} />);
    await userEvent.click(screen.getByText("photos"));
    expect(onSelectRoot).toHaveBeenCalledWith(1);
  });

  it("calls onDeleteRoot when delete button clicked", async () => {
    const onDeleteRoot = vi.fn();
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} onDeleteRoot={onDeleteRoot} />);
    await userEvent.click(screen.getByLabelText("Remove photos"));
    expect(onDeleteRoot).toHaveBeenCalledWith(sampleRoot);
  });

  it("shows db stats", () => {
    render(<Sidebar {...defaultProps} dbStats={{ files: 100, roots: 3, dbSizeBytes: 2048000, thumbsSizeBytes: 51200000 }} />);
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("2.0 MB")).toBeInTheDocument();
    expect(screen.getByText("48.8 MB")).toBeInTheDocument();
  });

  it("disables add button when setup not ready", () => {
    render(<Sidebar {...defaultProps} setupReady={false} />);
    const addBtn = screen.getByTitle("Add folder to scan");
    expect(addBtn).toBeDisabled();
  });

  it("hides add/delete buttons in readOnly mode", () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} readOnly />);
    expect(screen.queryByTitle("Add folder to scan")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Remove photos")).not.toBeInTheDocument();
  });

  it("renders running scan with pause button inside RootCard", () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} activeScans={[mockRunningScan]} />);
    expect(screen.getByText("50/100")).toBeInTheDocument();
    expect(screen.getByText("Pause")).toBeInTheDocument();
  });

  it("renders interrupted scan with resume button inside RootCard", () => {
    const interruptedScan = { ...mockRunningScan, id: 11, status: "interrupted" as const };
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} activeScans={[interruptedScan]} />);
    expect(screen.getByText("Scan interrupted")).toBeInTheDocument();
    expect(screen.getByText("Resume")).toBeInTheDocument();
  });

  it("shows context menu on right-click of root card", async () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} />);
    const card = screen.getByText("photos").closest(".root-card")!;
    await userEvent.pointer({ keys: "[MouseRight]", target: card });
    expect(screen.getByRole("menuitem", { name: "Rescan" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Remove" })).toBeInTheDocument();
  });

  it("calls onRescanRoot from context menu", async () => {
    const onRescanRoot = vi.fn();
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} onRescanRoot={onRescanRoot} />);
    const card = screen.getByText("photos").closest(".root-card")!;
    await userEvent.pointer({ keys: "[MouseRight]", target: card });
    await userEvent.click(screen.getByRole("menuitem", { name: "Rescan" }));
    expect(onRescanRoot).toHaveBeenCalledWith(sampleRoot);
  });

  it("closes context menu on Escape", async () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} />);
    const card = screen.getByText("photos").closest(".root-card")!;
    await userEvent.pointer({ keys: "[MouseRight]", target: card });
    expect(screen.getByRole("menuitem", { name: "Rescan" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menuitem", { name: "Rescan" })).not.toBeInTheDocument();
  });

  it("does not show context menu in readOnly mode", async () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} readOnly />);
    const card = screen.getByText("photos").closest(".root-card")!;
    await userEvent.pointer({ keys: "[MouseRight]", target: card });
    expect(screen.queryByRole("menuitem", { name: "Rescan" })).not.toBeInTheDocument();
  });

  it("root cards are draggable when not readOnly", () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} />);
    const card = screen.getByText("photos").closest(".root-card")!;
    expect(card.parentElement?.getAttribute("draggable")).toBe("true");
  });

  it("root cards are not draggable in readOnly mode", () => {
    render(<Sidebar {...defaultProps} roots={[sampleRoot]} readOnly />);
    const card = screen.getByText("photos").closest(".root-card")!;
    expect(card.parentElement?.getAttribute("draggable")).toBe("false");
  });
});
