import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RenameModal from "../../components/modals/RenameModal";

describe("RenameModal", () => {
  it("pre-fills input with current filename", () => {
    render(<RenameModal currentName="photo.jpg" onCancel={() => {}} onConfirm={() => {}} />);
    const input = screen.getByLabelText("New filename") as HTMLInputElement;
    expect(input.value).toBe("photo.jpg");
  });

  it("calls onCancel when Cancel clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<RenameModal currentName="photo.jpg" onCancel={onCancel} onConfirm={() => {}} />);
    await user.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onConfirm with new name when Rename clicked", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<RenameModal currentName="photo.jpg" onCancel={() => {}} onConfirm={onConfirm} />);
    const input = screen.getByLabelText("New filename");
    await user.clear(input);
    await user.type(input, "renamed.jpg");
    await user.click(screen.getByText("Rename"));
    expect(onConfirm).toHaveBeenCalledWith("renamed.jpg");
  });

  it("shows error when name is empty", async () => {
    const user = userEvent.setup();
    render(<RenameModal currentName="photo.jpg" onCancel={() => {}} onConfirm={() => {}} />);
    const input = screen.getByLabelText("New filename");
    await user.clear(input);
    await user.click(screen.getByText("Rename"));
    expect(screen.getByText("Filename cannot be empty")).toBeInTheDocument();
  });

  it("shows error when name contains path separator", async () => {
    const user = userEvent.setup();
    render(<RenameModal currentName="photo.jpg" onCancel={() => {}} onConfirm={() => {}} />);
    const input = screen.getByLabelText("New filename");
    await user.clear(input);
    await user.type(input, "sub/file.jpg");
    await user.click(screen.getByText("Rename"));
    expect(screen.getByText("Filename cannot contain path separators")).toBeInTheDocument();
  });

  it("shows error when name is unchanged", async () => {
    const user = userEvent.setup();
    render(<RenameModal currentName="photo.jpg" onCancel={() => {}} onConfirm={() => {}} />);
    await user.click(screen.getByText("Rename"));
    expect(screen.getByText("Name is unchanged")).toBeInTheDocument();
  });

  it("submits on Enter key", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<RenameModal currentName="photo.jpg" onCancel={() => {}} onConfirm={onConfirm} />);
    const input = screen.getByLabelText("New filename");
    await user.clear(input);
    await user.type(input, "new.jpg{Enter}");
    expect(onConfirm).toHaveBeenCalledWith("new.jpg");
  });
});
