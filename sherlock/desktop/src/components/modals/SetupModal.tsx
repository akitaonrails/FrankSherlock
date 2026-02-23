import type { SetupStatus } from "../../types";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./SetupModal.css";

type Props = {
  setup: SetupStatus;
  onRecheck: () => void;
  onDownload: () => void;
};

export default function SetupModal({ setup, onRecheck, onDownload }: Props) {
  return (
    <ModalOverlay>
      <div className="modal-base setup-modal">
        <h2>First-Time Setup</h2>
        <p>Sherlock needs local Ollama service and required model(s) before scanning.</p>
        <div className="setup-status-grid">
          <div>
            <strong>Ollama</strong>
            <p>{setup.ollamaAvailable ? "Running" : "Not detected"}</p>
          </div>
          <div>
            <strong>Required</strong>
            <p>{setup.requiredModels.join(", ")}</p>
          </div>
          <div>
            <strong>Missing</strong>
            <p>{setup.missingModels.length ? setup.missingModels.join(", ") : "None"}</p>
          </div>
          <div>
            <strong>OCR (Surya)</strong>
            <p>
              {setup.suryaVenvOk
                ? `Ready${setup.pythonVersion ? ` (Python ${setup.pythonVersion})` : ""}`
                : setup.pythonAvailable
                  ? "Python found, venv issue"
                  : "Not available"}
            </p>
          </div>
        </div>
        <ul className="setup-instructions">
          {setup.instructions.map((instruction) => (
            <li key={instruction}>{instruction}</li>
          ))}
        </ul>
        <div className="progress-wrap">
          <progress value={setup.download.progressPct} max={100} />
          <span>{setup.download.progressPct.toFixed(1)}%</span>
        </div>
        <p className="setup-download-text">{setup.download.message}</p>
        <div className="modal-actions">
          <button type="button" onClick={onRecheck}>Recheck</button>
          <button
            type="button"
            onClick={onDownload}
            disabled={
              !setup.ollamaAvailable ||
              setup.missingModels.length === 0 ||
              setup.download.status === "running"
            }
          >
            {setup.download.status === "running" ? "Downloading..." : "Download model"}
          </button>
        </div>
      </div>
    </ModalOverlay>
  );
}
