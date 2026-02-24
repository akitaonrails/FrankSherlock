import { useState, useRef, useCallback, useEffect } from "react";
import { Document, Page, pdfjs } from "react-pdf";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listPdfPasswords, addPdfPassword, reclassifyPdf } from "../../api";
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
  fileId?: number;
};

export default function PdfViewer({ filePath, className, fileId }: Props) {
  const [numPages, setNumPages] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState<number>(600);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [passwordInput, setPasswordInput] = useState("");
  const [passwordError, setPasswordError] = useState(false);

  const savedPasswordsRef = useRef<string[]>([]);
  const savedPasswordIndexRef = useRef(0);
  const passwordCallbackRef = useRef<((pw: string | null) => void) | null>(null);
  const manualPasswordRef = useRef<string | null>(null);
  const savedUnlockedRef = useRef(false);

  useEffect(() => {
    listPdfPasswords()
      .then((pws) => { savedPasswordsRef.current = pws.map((p) => p.password); })
      .catch(() => { savedPasswordsRef.current = []; });
  }, []);

  const onDocumentLoadSuccess = useCallback(
    ({ numPages: total }: { numPages: number }) => {
      setNumPages(total);
      setError(null);
      setNeedsPassword(false);
      setPasswordError(false);
      const wasUnlocked = manualPasswordRef.current || savedUnlockedRef.current;
      // Save manually entered password that worked
      if (manualPasswordRef.current) {
        const pw = manualPasswordRef.current;
        manualPasswordRef.current = null;
        addPdfPassword(pw, "").catch(() => {});
      }
      savedUnlockedRef.current = false;
      // Trigger reclassification so the file gets AI-classified and thumbnailed
      if (wasUnlocked && fileId != null) {
        reclassifyPdf(fileId).catch(() => {});
      }
      if (containerRef.current) {
        setContainerWidth(containerRef.current.clientWidth - 16);
      }
    },
    [],
  );

  const onDocumentLoadError = useCallback((err: Error) => {
    setError(err.message || "Failed to load PDF");
  }, []);

  const handlePassword = useCallback(
    (callback: (password: string | null) => void, reason: number) => {
      const NEED_PASSWORD = 1;
      if (reason === NEED_PASSWORD) {
        savedPasswordIndexRef.current = 0;
        manualPasswordRef.current = null;
        savedUnlockedRef.current = false;
      }

      // Auto-try saved passwords first
      const saved = savedPasswordsRef.current;
      const idx = savedPasswordIndexRef.current;
      if (idx < saved.length) {
        savedPasswordIndexRef.current = idx + 1;
        savedUnlockedRef.current = true;
        callback(saved[idx]);
        return;
      }

      // All saved passwords exhausted — show prompt
      if (manualPasswordRef.current) {
        setPasswordError(true);
      }
      manualPasswordRef.current = null;
      passwordCallbackRef.current = callback;
      setNeedsPassword(true);
      setPasswordInput("");
    },
    [],
  );

  function handlePasswordSubmit() {
    const pw = passwordInput.trim();
    if (!pw || !passwordCallbackRef.current) return;
    manualPasswordRef.current = pw;
    passwordCallbackRef.current(pw);
    setNeedsPassword(false);
    setPasswordError(false);
  }

  function handlePasswordKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") handlePasswordSubmit();
  }

  const fileUrl = convertFileSrc(filePath);

  return (
    <div
      ref={containerRef}
      className={`pdf-viewer${className ? ` ${className}` : ""}`}
    >
      {error && <div className="pdf-viewer-error">Error: {error}</div>}
      {needsPassword && (
        <div className="pdf-viewer-password">
          <p>{passwordError ? "Incorrect password. Try again:" : "This PDF is password-protected:"}</p>
          <div className="pdf-viewer-password-form">
            <input
              type="password"
              value={passwordInput}
              onChange={(e) => setPasswordInput(e.target.value)}
              onKeyDown={handlePasswordKeyDown}
              placeholder="Enter password"
              autoFocus
            />
            <button type="button" onClick={handlePasswordSubmit}>Unlock</button>
          </div>
        </div>
      )}
      <Document
        file={fileUrl}
        onLoadSuccess={onDocumentLoadSuccess}
        onLoadError={onDocumentLoadError}
        onPassword={handlePassword}
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
