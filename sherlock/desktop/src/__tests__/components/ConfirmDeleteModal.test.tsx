import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ConfirmDeleteModal from "../../components/modals/ConfirmDeleteModal";
import { mockRoot as root } from "../fixtures";

describe("ConfirmDeleteModal", () => {
  it("renders folder name and file count", () => {
    render(<ConfirmDeleteModal root={root} onCancel={() => {}} onConfirm={() => {}} />);
    expect(screen.getByText("photos")).toBeInTheDocument();
    expect(screen.getByText(/42 indexed files/)).toBeInTheDocument();
  });

  it("shows the full path", () => {
    render(<ConfirmDeleteModal root={root} onCancel={() => {}} onConfirm={() => {}} />);
    expect(screen.getByText("/home/user/photos")).toBeInTheDocument();
  });

  it("calls onCancel when Cancel clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<ConfirmDeleteModal root={root} onCancel={onCancel} onConfirm={() => {}} />);
    await user.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onConfirm with root when Remove clicked", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<ConfirmDeleteModal root={root} onCancel={() => {}} onConfirm={onConfirm} />);
    await user.click(screen.getByText("Remove"));
    expect(onConfirm).toHaveBeenCalledWith(root);
  });
});
