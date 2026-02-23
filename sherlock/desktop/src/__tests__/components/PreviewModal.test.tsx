import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import PreviewModal from "../../components/modals/PreviewModal";
import { mockSearchItem } from "../fixtures";

const item = {
  ...mockSearchItem,
  relPath: "photos/sunset.jpg",
  absPath: "/home/user/photos/sunset.jpg",
  description: "A beautiful sunset",
  confidence: 0.92,
  sizeBytes: 2048,
};

describe("PreviewModal", () => {
  it("renders single image with details", () => {
    render(
      <PreviewModal
        previewItems={[item]}
        selectedCount={1}
        singlePreviewIndex={0}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />
    );
    expect(screen.getByText("photos/sunset.jpg")).toBeInTheDocument();
    expect(screen.getByText("A beautiful sunset")).toBeInTheDocument();
    expect(screen.getByText("2 KB")).toBeInTheDocument();
  });

  it("calls onClose when close button clicked", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <PreviewModal
        previewItems={[item]}
        selectedCount={1}
        singlePreviewIndex={0}
        totalItems={10}
        onClose={onClose}
        onNavigate={() => {}}
      />
    );
    await user.click(screen.getByLabelText("Close preview"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("shows nav buttons for single image not at edges", () => {
    render(
      <PreviewModal
        previewItems={[item]}
        selectedCount={1}
        singlePreviewIndex={5}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />
    );
    expect(screen.getByLabelText("Previous image")).toBeInTheDocument();
    expect(screen.getByLabelText("Next image")).toBeInTheDocument();
  });

  it("hides prev button at first item", () => {
    render(
      <PreviewModal
        previewItems={[item]}
        selectedCount={1}
        singlePreviewIndex={0}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />
    );
    expect(screen.queryByLabelText("Previous image")).toBeNull();
    expect(screen.getByLabelText("Next image")).toBeInTheDocument();
  });

  it("renders collage for multiple items", () => {
    const items = [item, { ...item, id: 2, relPath: "photos/beach.jpg", absPath: "/home/user/photos/beach.jpg" }];
    const { container } = render(
      <PreviewModal
        previewItems={items}
        selectedCount={2}
        singlePreviewIndex={null}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />
    );
    expect(container.querySelector(".preview-collage")).not.toBeNull();
    expect(screen.getByText("2 files selected")).toBeInTheDocument();
  });
});
