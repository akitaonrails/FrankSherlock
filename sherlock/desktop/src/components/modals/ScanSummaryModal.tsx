import type { ScanJobStatus } from "../../types";
import { basename } from "../../utils";
import { formatElapsed } from "../../utils/format";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./ScanSummaryModal.css";

type Props = {
  completedJobs: ScanJobStatus[];
  onClose: () => void;
};

export default function ScanSummaryModal({ completedJobs, onClose }: Props) {
  return (
    <ModalOverlay>
      <div className="modal-base summary-modal">
        <h2>Scan Complete</h2>
        <table className="summary-table">
          <thead>
            <tr>
              <th>Folder</th>
              <th>Files</th>
              <th>Time</th>
            </tr>
          </thead>
          <tbody>
            {completedJobs.map((job) => (
              <tr key={job.id}>
                <td title={job.rootPath}>{basename(job.rootPath)}</td>
                <td>{job.processedFiles}</td>
                <td>{formatElapsed(job.startedAt, job.completedAt)}</td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr>
              <td><strong>Total</strong></td>
              <td><strong>{completedJobs.reduce((s, j) => s + j.processedFiles, 0)}</strong></td>
              <td>
                <strong>
                  {formatElapsed(
                    Math.min(...completedJobs.map((j) => j.startedAt)),
                    Math.max(...completedJobs.map((j) => j.completedAt ?? j.updatedAt))
                  )}
                </strong>
              </td>
            </tr>
          </tfoot>
        </table>
        <div className="modal-actions">
          <button type="button" onClick={onClose}>Close</button>
        </div>
      </div>
    </ModalOverlay>
  );
}
