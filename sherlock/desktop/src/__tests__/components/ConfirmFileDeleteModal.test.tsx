import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ConfirmFileDeleteModal from "../../components/modals/ConfirmFileDeleteModal";
import { mockSearchItem } from "../fixtures";

const makeFiles = (n: number) =>
  Array.from({ length: n }, (_, i) => ({
    ...mockSearchItem,
    id: i + 1,
    relPath: `photos/file${i + 1}.jpg`,
  }));

describe("ConfirmFileDeleteModal", () => {
  it("shows file count in title", () => {
    render(<ConfirmFileDeleteModal files={makeFiles(3)} onCancel={() => {}} onConfirm={() => {}} />);
    expect(screen.getByText("Delete 3 files?")).toBeInTheDocument();
  });

  it("shows singular for 1 file", () => {
    render(<ConfirmFileDeleteModal files={makeFiles(1)} onCancel={() => {}} onConfirm={() => {}} />);
    expect(screen.getByText("Delete 1 file?")).toBeInTheDocument();
  });

  it("shows first 5 filenames and '+ N more...' for larger lists", () => {
    render(<ConfirmFileDeleteModal files={makeFiles(8)} onCancel={() => {}} onConfirm={() => {}} />);
    expect(screen.getByText("file1.jpg")).toBeInTheDocument();
    expect(screen.getByText("file5.jpg")).toBeInTheDocument();
    expect(screen.queryByText("file6.jpg")).toBeNull();
    expect(screen.getByText("+ 3 more...")).toBeInTheDocument();
  });

  it("calls onCancel when Cancel clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<ConfirmFileDeleteModal files={makeFiles(1)} onCancel={onCancel} onConfirm={() => {}} />);
    await user.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onConfirm when Delete clicked", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<ConfirmFileDeleteModal files={makeFiles(2)} onCancel={() => {}} onConfirm={onConfirm} />);
    await user.click(screen.getByText("Delete"));
    expect(onConfirm).toHaveBeenCalledOnce();
  });
});
