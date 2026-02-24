import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import PreviewModal from "../../components/modals/PreviewModal";
import { mockSearchItem } from "../fixtures";

// Mock react-pdf to avoid needing pdf.js worker in tests
vi.mock("react-pdf", () => ({
  Document: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="pdf-document">{children}</div>
  ),
  Page: ({ pageNumber }: { pageNumber: number }) => (
    <div data-testid="pdf-page">Page {pageNumber}</div>
  ),
  pdfjs: { GlobalWorkerOptions: { workerSrc: "" } },
}));

// Mock convertFileSrc and invoke for Tauri
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: vi.fn().mockResolvedValue([]),
}));

const item = {
  ...mockSearchItem,
  relPath: "photos/sunset.jpg",
  absPath: "/home/user/photos/sunset.jpg",
  description: "A beautiful sunset",
  confidence: 0.92,
  sizeBytes: 2048,
};

const pdfItem = {
  ...mockSearchItem,
  id: 10,
  relPath: "docs/report.pdf",
  absPath: "/home/user/docs/report.pdf",
  mediaType: "document",
  description: "PDF document (5 pages)",
  confidence: 0.7,
  sizeBytes: 51200,
};

const pdfItem2 = {
  ...pdfItem,
  id: 11,
  relPath: "docs/invoice.PDF",
  absPath: "/home/user/docs/invoice.PDF",
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
      />,
    );
    expect(screen.getByText("photos/sunset.jpg")).toBeInTheDocument();
    expect(screen.getByText("A beautiful sunset")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
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
      />,
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
      />,
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
      />,
    );
    expect(screen.queryByLabelText("Previous image")).toBeNull();
    expect(screen.getByLabelText("Next image")).toBeInTheDocument();
  });

  it("renders collage for multiple images with comparison list", () => {
    const items = [
      item,
      {
        ...item,
        id: 2,
        relPath: "photos/beach.jpg",
        absPath: "/home/user/photos/beach.jpg",
      },
    ];
    const { container } = render(
      <PreviewModal
        previewItems={items}
        selectedCount={2}
        singlePreviewIndex={null}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    expect(container.querySelector(".preview-collage")).not.toBeNull();
    // Shows per-file comparison rows instead of generic count
    expect(container.querySelector(".preview-compare-list")).not.toBeNull();
    expect(screen.getByText("sunset.jpg")).toBeInTheDocument();
    expect(screen.getByText("beach.jpg")).toBeInTheDocument();
  });

  it("renders single PDF with PdfViewer", () => {
    render(
      <PreviewModal
        previewItems={[pdfItem]}
        selectedCount={1}
        singlePreviewIndex={0}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    const { container } = render(
      <PreviewModal
        previewItems={[pdfItem]}
        selectedCount={1}
        singlePreviewIndex={0}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    expect(container.querySelector(".preview-pdf-wrap")).not.toBeNull();
    expect(screen.getAllByTestId("pdf-document").length).toBeGreaterThanOrEqual(
      1,
    );
  });

  it("renders 2 PDFs side-by-side", () => {
    const { container } = render(
      <PreviewModal
        previewItems={[pdfItem, pdfItem2]}
        selectedCount={2}
        singlePreviewIndex={null}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    expect(screen.getByTestId("preview-split")).toBeInTheDocument();
    const panes = container.querySelectorAll(".preview-split-pane");
    expect(panes.length).toBe(2);
    expect(screen.getAllByTestId("pdf-document").length).toBe(2);
  });

  it("renders PDF + image in split layout", () => {
    const { container } = render(
      <PreviewModal
        previewItems={[pdfItem, item]}
        selectedCount={2}
        singlePreviewIndex={null}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    expect(screen.getByTestId("preview-split")).toBeInTheDocument();
    const panes = container.querySelectorAll(".preview-split-pane");
    expect(panes.length).toBe(2);
    // One pane has PDF, other has image
    expect(screen.getAllByTestId("pdf-document").length).toBe(1);
    expect(container.querySelector(".preview-split-pane img")).not.toBeNull();
  });

  it("caps PDFs at 2 when 3+ selected", () => {
    const pdfItem3 = { ...pdfItem, id: 12, relPath: "docs/third.pdf" };
    const { container } = render(
      <PreviewModal
        previewItems={[pdfItem, pdfItem2, pdfItem3]}
        selectedCount={3}
        singlePreviewIndex={null}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    // Should show dual-pdf layout (capped at 2)
    expect(screen.getByTestId("preview-split")).toBeInTheDocument();
    expect(screen.getAllByTestId("pdf-document").length).toBe(2);
    const panes = container.querySelectorAll(".preview-split-pane");
    expect(panes.length).toBe(2);
  });

  it("detects PDF with uppercase extension", () => {
    const upperPdf = { ...pdfItem, relPath: "docs/REPORT.PDF" };
    const { container } = render(
      <PreviewModal
        previewItems={[upperPdf]}
        selectedCount={1}
        singlePreviewIndex={0}
        totalItems={10}
        onClose={() => {}}
        onNavigate={() => {}}
      />,
    );
    expect(container.querySelector(".preview-pdf-wrap")).not.toBeNull();
  });
});
