import type { RootInfo } from "../../types";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./ConfirmDeleteModal.css";

type Props = {
  root: RootInfo;
  onCancel: () => void;
  onConfirm: (root: RootInfo) => void;
};

export default function ConfirmDeleteModal({ root, onCancel, onConfirm }: Props) {
  return (
    <ModalOverlay onBackdropClick={onCancel}>
      <div className="modal-base confirm-modal" onClick={(e) => e.stopPropagation()}>
        <h3>Remove folder?</h3>
        <p>
          This will remove <strong>{root.rootName}</strong> and
          all {root.fileCount} indexed files from the database and cache.
        </p>
        <p className="confirm-path">{root.rootPath}</p>
        <p className="confirm-note">Original files on disk will not be touched.</p>
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>Cancel</button>
          <button type="button" className="danger-btn" onClick={() => onConfirm(root)}>
            Remove
          </button>
        </div>
      </div>
    </ModalOverlay>
  );
}
