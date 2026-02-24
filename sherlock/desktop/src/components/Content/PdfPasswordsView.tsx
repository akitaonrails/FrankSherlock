import { useEffect, useState } from "react";
import type { PdfPassword, ProtectedPdfInfo } from "../../types";
import {
  addPdfPassword,
  deletePdfPassword,
  listPdfPasswords,
  listProtectedPdfs,
  retryProtectedPdfs,
} from "../../api";
import { basename } from "../../utils";
import "./PdfPasswordsView.css";

type Props = {
  onBack: () => void;
  onNotice: (msg: string) => void;
  onError: (msg: string) => void;
};

export default function PdfPasswordsView({ onBack, onNotice, onError }: Props) {
  const [passwords, setPasswords] = useState<PdfPassword[]>([]);
  const [protectedPdfs, setProtectedPdfs] = useState<ProtectedPdfInfo[]>([]);
  const [newPassword, setNewPassword] = useState("");
  const [newLabel, setNewLabel] = useState("");
  const [revealed, setRevealed] = useState<Set<number>>(new Set());
  const [retrying, setRetrying] = useState(false);

  async function refresh() {
    try {
      const [pw, pdfs] = await Promise.all([listPdfPasswords(), listProtectedPdfs()]);
      setPasswords(pw);
      setProtectedPdfs(pdfs);
    } catch (err) {
      onError(String(err));
    }
  }

  useEffect(() => { refresh(); }, []);

  async function handleAdd() {
    const pw = newPassword.trim();
    if (!pw) return;
    try {
      await addPdfPassword(pw, newLabel.trim());
      setNewPassword("");
      setNewLabel("");
      await refresh();
      onNotice("Password saved");
    } catch (err) {
      onError(String(err));
    }
  }

  async function handleDelete(id: number) {
    try {
      await deletePdfPassword(id);
      setRevealed((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
      await refresh();
    } catch (err) {
      onError(String(err));
    }
  }

  function toggleReveal(id: number) {
    setRevealed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function handleRetry() {
    if (passwords.length === 0 || protectedPdfs.length === 0) return;
    setRetrying(true);
    try {
      const result = await retryProtectedPdfs();
      onNotice(`Unlocked ${result.unlocked} of ${result.totalAttempted} PDF(s)`);
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setRetrying(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") handleAdd();
  }

  return (
    <div className="pdf-passwords-view">
      <div className="pdf-passwords-toolbar">
        <div className="pdf-passwords-stats">
          <strong>{passwords.length}</strong> saved password{passwords.length !== 1 ? "s" : ""},
          {" "}<strong>{protectedPdfs.length}</strong> protected PDF{protectedPdfs.length !== 1 ? "s" : ""}
        </div>
        <button type="button" onClick={onBack}>Back</button>
      </div>

      <div className="pdf-passwords-body">
        <div className="pdf-passwords-warning">
          Passwords are stored in plain text in the local database.
          Only save passwords you are comfortable storing unencrypted.
        </div>

        {/* ── Saved Passwords ── */}
        <div className="pdf-passwords-section-header">Saved Passwords</div>

        {passwords.length === 0 ? (
          <div className="pdf-passwords-empty">No passwords saved yet.</div>
        ) : (
          passwords.map((pw) => (
            <div key={pw.id} className="pdf-password-row">
              <span className="pdf-password-value">
                {revealed.has(pw.id) ? pw.password : "\u2022".repeat(Math.min(pw.password.length, 20))}
              </span>
              {pw.label && <span className="pdf-password-label">{pw.label}</span>}
              <button type="button" onClick={() => toggleReveal(pw.id)}>
                {revealed.has(pw.id) ? "Hide" : "Show"}
              </button>
              <button type="button" onClick={() => handleDelete(pw.id)}>Delete</button>
            </div>
          ))
        )}

        <div className="pdf-password-add-form">
          <input
            type="text"
            placeholder="Password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            onKeyDown={handleKeyDown}
          />
          <input
            type="text"
            placeholder="Label (optional)"
            value={newLabel}
            onChange={(e) => setNewLabel(e.target.value)}
            onKeyDown={handleKeyDown}
          />
          <button
            type="button"
            onClick={handleAdd}
            disabled={!newPassword.trim()}
          >
            Save
          </button>
        </div>

        {/* ── Protected PDFs ── */}
        <div className="pdf-passwords-section-header">Protected PDFs</div>

        <div className="pdf-retry-area">
          <button
            type="button"
            onClick={handleRetry}
            disabled={retrying || passwords.length === 0 || protectedPdfs.length === 0}
          >
            {retrying ? "Retrying\u2026" : "Retry All"}
          </button>
          {passwords.length === 0 && protectedPdfs.length > 0 && (
            <span style={{ fontSize: "var(--font-size-sm)", color: "var(--text-secondary)" }}>
              Add a password first
            </span>
          )}
        </div>

        {protectedPdfs.length === 0 ? (
          <div className="pdf-passwords-empty">No password-protected PDFs found.</div>
        ) : (
          protectedPdfs.map((pdf) => (
            <div key={pdf.id} className="pdf-protected-row">
              <span className="pdf-protected-filename" title={pdf.absPath}>
                {basename(pdf.relPath)}
              </span>
              <span className="pdf-protected-root" title={pdf.rootPath}>
                {basename(pdf.rootPath)}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
