import type { SearchItem } from "../../types";
import { fileName } from "../../utils/format";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./ConfirmFileDeleteModal.css";

type Props = {
  files: SearchItem[];
  onCancel: () => void;
  onConfirm: () => void;
};

const MAX_SHOWN = 5;

export default function ConfirmFileDeleteModal({ files, onCancel, onConfirm }: Props) {
  const shown = files.slice(0, MAX_SHOWN);
  const remaining = files.length - shown.length;

  return (
    <ModalOverlay onBackdropClick={onCancel}>
      <div className="modal-base confirm-file-delete-modal" onClick={(e) => e.stopPropagation()}>
        <h3>Delete {files.length} file{files.length !== 1 ? "s" : ""}?</h3>
        <p className="file-delete-warning">
          This will permanently delete the selected file{files.length !== 1 ? "s" : ""} from disk.
        </p>
        <div className="file-delete-list">
          {shown.map((f) => (
            <div key={f.id}>{fileName(f.relPath)}</div>
          ))}
          {remaining > 0 && (
            <div className="file-delete-more">+ {remaining} more...</div>
          )}
        </div>
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>Cancel</button>
          <button type="button" className="danger-btn" onClick={onConfirm}>
            Delete
          </button>
        </div>
      </div>
    </ModalOverlay>
  );
}
