import type { ScanJobStatus } from "../../types";
import { basename } from "../../utils";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./ResumeModal.css";

type Props = {
  interruptedScans: ScanJobStatus[];
  onDismiss: () => void;
  onResumeAll: () => void;
};

export default function ResumeModal({ interruptedScans, onDismiss, onResumeAll }: Props) {
  return (
    <ModalOverlay>
      <div className="modal-base resume-modal">
        <h2>Interrupted Scans</h2>
        <p>The following scans were interrupted and can be resumed:</p>
        <ul className="resume-scan-list">
          {interruptedScans.map((scan) => (
            <li key={scan.id}>
              <strong>{basename(scan.rootPath)}</strong>
              <span> — {scan.processedFiles}/{scan.totalFiles} files processed</span>
            </li>
          ))}
        </ul>
        <div className="modal-actions">
          <button type="button" onClick={onDismiss}>Later</button>
          <button type="button" onClick={onResumeAll}>Resume Now</button>
        </div>
      </div>
    </ModalOverlay>
  );
}
