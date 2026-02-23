import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Sidebar from "../../components/Sidebar/Sidebar";
import type { RootInfo, ScanJobStatus } from "../../types";
import { mockRoot as sampleRoot, mockRunningScan } from "../fixtures";

const defaultProps = {
  roots: [] as RootInfo[],
  selectedRootId: null,
  activeScans: [] as ScanJobStatus[],
  runtime: null,
  dbStats: null,
  readOnly: false,
  setupReady: true,
  isScanning: false,
  onSelectRoot: vi.fn(),
  onDeleteRoot: vi.fn(),
  onPickAndScan: vi.fn(),
  onCancelScan: vi.fn(),
  onResumeScan: vi.fn(),
  onCleanupOllama: vi.fn(),
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
    render(<Sidebar {...defaultProps} dbStats={{ files: 100, roots: 3 }} />);
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
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

  it("renders running scan progress", () => {
    render(<Sidebar {...defaultProps} activeScans={[mockRunningScan]} />);
    expect(screen.getByText(/photos:.*50.*\/.*100/)).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("renders interrupted scan with resume button", () => {
    const interruptedScan = { ...mockRunningScan, id: 11, status: "interrupted" as const };
    render(<Sidebar {...defaultProps} activeScans={[interruptedScan]} />);
    expect(screen.getByText("Resume")).toBeInTheDocument();
  });
});
