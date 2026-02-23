import { useEffect, useRef, useState } from "react";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./RenameModal.css";

type Props = {
  currentName: string;
  onCancel: () => void;
  onConfirm: (newName: string) => void;
};

function validate(name: string, currentName: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "Filename cannot be empty";
  if (trimmed.includes("/") || trimmed.includes("\\")) return "Filename cannot contain path separators";
  if (trimmed === currentName) return "Name is unchanged";
  return null;
}

export default function RenameModal({ currentName, onCancel, onConfirm }: Props) {
  const [value, setValue] = useState(currentName);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.focus();
    // Select stem only (before last dot)
    const lastDot = currentName.lastIndexOf(".");
    el.setSelectionRange(0, lastDot > 0 ? lastDot : currentName.length);
  }, [currentName]);

  function handleSubmit() {
    const err = validate(value, currentName);
    if (err) {
      setError(err);
      return;
    }
    onConfirm(value.trim());
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSubmit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    }
  }

  return (
    <ModalOverlay onBackdropClick={onCancel}>
      <div className="modal-base rename-modal" onClick={(e) => e.stopPropagation()}>
        <h3>Rename file</h3>
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={(e) => { setValue(e.target.value); setError(null); }}
          onKeyDown={handleKeyDown}
          aria-label="New filename"
        />
        {error && <p className="rename-error">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>Cancel</button>
          <button type="button" onClick={handleSubmit}>Rename</button>
        </div>
      </div>
    </ModalOverlay>
  );
}
