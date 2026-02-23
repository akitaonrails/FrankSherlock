import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ContextMenu from "../../components/Content/ContextMenu";
import type { Album } from "../../types";

const baseProps = {
  x: 100,
  y: 200,
  selectedCount: 1,
  albums: [] as Album[],
  description: null as string | null,
  extractedText: null as string | null,
  onCopyPath: vi.fn(),
  onCopyDescription: vi.fn(),
  onCopyOcrText: vi.fn(),
  onRename: vi.fn(),
  onEditMetadata: vi.fn(),
  onDelete: vi.fn(),
  onAddToAlbum: vi.fn(),
  onCreateAlbumFromSelection: vi.fn(),
  onClose: vi.fn(),
};

describe("ContextMenu", () => {
  it("renders Copy Path and Delete for any selection", () => {
    render(<ContextMenu {...baseProps} selectedCount={3} />);
    expect(screen.getByText("Copy Path")).toBeInTheDocument();
    expect(screen.getByText("Delete")).toBeInTheDocument();
  });

  it("shows Rename only when exactly 1 file selected", () => {
    const { rerender } = render(<ContextMenu {...baseProps} selectedCount={1} />);
    expect(screen.getByText("Rename")).toBeInTheDocument();

    rerender(<ContextMenu {...baseProps} selectedCount={2} />);
    expect(screen.queryByText("Rename")).toBeNull();
  });

  it("calls onCopyPath when Copy Path clicked", async () => {
    const user = userEvent.setup();
    const onCopyPath = vi.fn();
    render(<ContextMenu {...baseProps} onCopyPath={onCopyPath} />);
    await user.click(screen.getByText("Copy Path"));
    expect(onCopyPath).toHaveBeenCalledOnce();
  });

  it("calls onDelete when Delete clicked", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    render(<ContextMenu {...baseProps} onDelete={onDelete} />);
    await user.click(screen.getByText("Delete"));
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it("calls onRename when Rename clicked", async () => {
    const user = userEvent.setup();
    const onRename = vi.fn();
    render(<ContextMenu {...baseProps} onRename={onRename} />);
    await user.click(screen.getByText("Rename"));
    expect(onRename).toHaveBeenCalledOnce();
  });

  it("shows keyboard shortcut hints", () => {
    render(<ContextMenu {...baseProps} />);
    expect(screen.getByText("Ctrl+C")).toBeInTheDocument();
    expect(screen.getByText("F2")).toBeInTheDocument();
    expect(screen.getByText("Del")).toBeInTheDocument();
  });

  it("has menu role", () => {
    render(<ContextMenu {...baseProps} />);
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  it("shows Edit Metadata only when exactly 1 file selected", () => {
    const { rerender } = render(<ContextMenu {...baseProps} selectedCount={1} />);
    expect(screen.getByText("Edit Metadata")).toBeInTheDocument();

    rerender(<ContextMenu {...baseProps} selectedCount={2} />);
    expect(screen.queryByText("Edit Metadata")).toBeNull();
  });

  it("calls onEditMetadata when Edit Metadata clicked", async () => {
    const user = userEvent.setup();
    const onEditMetadata = vi.fn();
    render(<ContextMenu {...baseProps} selectedCount={1} onEditMetadata={onEditMetadata} />);
    await user.click(screen.getByText("Edit Metadata"));
    expect(onEditMetadata).toHaveBeenCalledOnce();
  });

  it("shows Add to Album submenu", () => {
    render(<ContextMenu {...baseProps} />);
    expect(screen.getByText("Add to Album")).toBeInTheDocument();
    expect(screen.getByText("New Album...")).toBeInTheDocument();
  });

  it("shows Copy Description when single file has description", () => {
    render(<ContextMenu {...baseProps} selectedCount={1} description="A nice photo" />);
    expect(screen.getByText("Copy Description")).toBeInTheDocument();
  });

  it("hides Copy Description when no description", () => {
    render(<ContextMenu {...baseProps} selectedCount={1} description={null} />);
    expect(screen.queryByText("Copy Description")).toBeNull();
  });

  it("hides Copy Description for multi-selection", () => {
    render(<ContextMenu {...baseProps} selectedCount={3} description="A nice photo" />);
    expect(screen.queryByText("Copy Description")).toBeNull();
  });

  it("shows Copy OCR Text when single file has extractedText", () => {
    render(<ContextMenu {...baseProps} selectedCount={1} extractedText="Some OCR text" />);
    expect(screen.getByText("Copy OCR Text")).toBeInTheDocument();
  });

  it("hides Copy OCR Text when no extractedText", () => {
    render(<ContextMenu {...baseProps} selectedCount={1} extractedText={null} />);
    expect(screen.queryByText("Copy OCR Text")).toBeNull();
  });

  it("hides Copy OCR Text for empty string extractedText", () => {
    render(<ContextMenu {...baseProps} selectedCount={1} extractedText="" />);
    expect(screen.queryByText("Copy OCR Text")).toBeNull();
  });

  it("calls onCopyDescription when clicked", async () => {
    const user = userEvent.setup();
    const onCopyDescription = vi.fn();
    render(<ContextMenu {...baseProps} selectedCount={1} description="desc" onCopyDescription={onCopyDescription} />);
    await user.click(screen.getByText("Copy Description"));
    expect(onCopyDescription).toHaveBeenCalledOnce();
  });

  it("calls onCopyOcrText when clicked", async () => {
    const user = userEvent.setup();
    const onCopyOcrText = vi.fn();
    render(<ContextMenu {...baseProps} selectedCount={1} extractedText="ocr" onCopyOcrText={onCopyOcrText} />);
    await user.click(screen.getByText("Copy OCR Text"));
    expect(onCopyOcrText).toHaveBeenCalledOnce();
  });
});
