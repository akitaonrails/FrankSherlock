import { useState, useRef, useCallback } from "react";
import { Document, Page, pdfjs } from "react-pdf";
import { convertFileSrc } from "@tauri-apps/api/core";
import "react-pdf/dist/Page/AnnotationLayer.css";
import "react-pdf/dist/Page/TextLayer.css";
import "./PdfViewer.css";

// Configure pdf.js worker
pdfjs.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url,
).toString();

type Props = {
  filePath: string;
  className?: string;
};

export default function PdfViewer({ filePath, className }: Props) {
  const [numPages, setNumPages] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState<number>(600);

  const onDocumentLoadSuccess = useCallback(
    ({ numPages: total }: { numPages: number }) => {
      setNumPages(total);
      setError(null);
      // Measure container width for responsive page sizing
      if (containerRef.current) {
        setContainerWidth(containerRef.current.clientWidth - 16);
      }
    },
    [],
  );

  const onDocumentLoadError = useCallback((err: Error) => {
    setError(err.message || "Failed to load PDF");
  }, []);

  const fileUrl = convertFileSrc(filePath);

  return (
    <div
      ref={containerRef}
      className={`pdf-viewer${className ? ` ${className}` : ""}`}
    >
      {error && <div className="pdf-viewer-error">Error: {error}</div>}
      <Document
        file={fileUrl}
        onLoadSuccess={onDocumentLoadSuccess}
        onLoadError={onDocumentLoadError}
        loading={<div className="pdf-viewer-loading">Loading PDF...</div>}
      >
        {Array.from({ length: numPages }, (_, i) => (
          <div key={i} className="pdf-viewer-page">
            <Page
              pageNumber={i + 1}
              width={containerWidth}
              renderTextLayer={true}
              renderAnnotationLayer={true}
            />
            <div className="pdf-viewer-page-label">
              Page {i + 1} of {numPages}
            </div>
          </div>
        ))}
      </Document>
    </div>
  );
}
