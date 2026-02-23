import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ContextMenu from "../../components/Content/ContextMenu";

const baseProps = {
  x: 100,
  y: 200,
  selectedCount: 1,
  onCopy: vi.fn(),
  onRename: vi.fn(),
  onDelete: vi.fn(),
  onClose: vi.fn(),
};

describe("ContextMenu", () => {
  it("renders Copy and Delete for any selection", () => {
    render(<ContextMenu {...baseProps} selectedCount={3} />);
    expect(screen.getByText("Copy")).toBeInTheDocument();
    expect(screen.getByText("Delete")).toBeInTheDocument();
  });

  it("shows Rename only when exactly 1 file selected", () => {
    const { rerender } = render(<ContextMenu {...baseProps} selectedCount={1} />);
    expect(screen.getByText("Rename")).toBeInTheDocument();

    rerender(<ContextMenu {...baseProps} selectedCount={2} />);
    expect(screen.queryByText("Rename")).toBeNull();
  });

  it("calls onCopy when Copy clicked", async () => {
    const user = userEvent.setup();
    const onCopy = vi.fn();
    render(<ContextMenu {...baseProps} onCopy={onCopy} />);
    await user.click(screen.getByText("Copy"));
    expect(onCopy).toHaveBeenCalledOnce();
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
});
