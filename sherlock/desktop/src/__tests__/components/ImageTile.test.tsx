import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ImageTile from "../../components/Content/ImageTile";
import { mockSearchItem as mockItem } from "../fixtures";

describe("ImageTile", () => {
  it("renders the file name", () => {
    render(
      <ImageTile item={mockItem} index={0} isSelected={false} isFocused={false} onClick={() => {}} onDoubleClick={() => {}} onContextMenu={() => {}} />
    );
    expect(screen.getAllByText("beach.jpg").length).toBeGreaterThan(0);
  });

  it("renders thumbnail image when available", () => {
    render(
      <ImageTile item={mockItem} index={0} isSelected={false} isFocused={false} onClick={() => {}} onDoubleClick={() => {}} onContextMenu={() => {}} />
    );
    const img = screen.getByAltText("photos/beach.jpg");
    expect(img).toBeInTheDocument();
    expect(img.getAttribute("src")).toContain("thumb.jpg");
  });

  it("renders placeholder when no thumbnail", () => {
    const noThumb = { ...mockItem, thumbnailPath: null };
    const { container } = render(
      <ImageTile item={noThumb} index={0} isSelected={false} isFocused={false} onClick={() => {}} onDoubleClick={() => {}} onContextMenu={() => {}} />
    );
    expect(container.querySelector(".tile-thumb-placeholder .badge")).toHaveTextContent("photo");
  });

  it("applies selected class", () => {
    const { container } = render(
      <ImageTile item={mockItem} index={0} isSelected={true} isFocused={false} onClick={() => {}} onDoubleClick={() => {}} onContextMenu={() => {}} />
    );
    expect(container.querySelector(".tile-selected")).not.toBeNull();
  });

  it("calls onClick with index and event", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(
      <ImageTile item={mockItem} index={3} isSelected={false} isFocused={false} onClick={onClick} onDoubleClick={() => {}} onContextMenu={() => {}} />
    );
    await user.click(screen.getByRole("listitem"));
    expect(onClick).toHaveBeenCalledWith(3, expect.any(Object));
  });

  it("calls onDoubleClick with index", async () => {
    const user = userEvent.setup();
    const onDoubleClick = vi.fn();
    render(
      <ImageTile item={mockItem} index={2} isSelected={false} isFocused={false} onClick={() => {}} onDoubleClick={onDoubleClick} onContextMenu={() => {}} />
    );
    await user.dblClick(screen.getByRole("listitem"));
    expect(onDoubleClick).toHaveBeenCalledWith(2);
  });
});
