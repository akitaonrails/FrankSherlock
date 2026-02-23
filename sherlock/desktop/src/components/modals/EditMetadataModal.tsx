import { useEffect, useState } from "react";
import { getFileMetadata } from "../../api";
import type { FileMetadata } from "../../types";
import { errorMessage } from "../../utils";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./EditMetadataModal.css";

const MEDIA_TYPES = ["photo", "anime", "manga", "screenshot", "document", "artwork", "other"];

type Props = {
  fileId: number;
  onSave: (data: FileMetadata) => void;
  onCancel: () => void;
};

export default function EditMetadataModal({ fileId, onSave, onCancel }: Props) {
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [mediaType, setMediaType] = useState("");
  const [description, setDescription] = useState("");
  const [extractedText, setExtractedText] = useState("");
  const [canonicalMentions, setCanonicalMentions] = useState("");
  const [locationText, setLocationText] = useState("");

  useEffect(() => {
    let cancelled = false;
    getFileMetadata(fileId)
      .then((meta) => {
        if (cancelled) return;
        setMediaType(meta.mediaType);
        setDescription(meta.description);
        setExtractedText(meta.extractedText);
        setCanonicalMentions(meta.canonicalMentions);
        setLocationText(meta.locationText);
        setLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        setLoadError(errorMessage(err));
        setLoading(false);
      });
    return () => { cancelled = true; };
  }, [fileId]);

  function handleSave() {
    onSave({
      id: fileId,
      mediaType,
      description,
      extractedText,
      canonicalMentions,
      locationText,
    });
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    }
  }

  return (
    <ModalOverlay onBackdropClick={onCancel}>
      <div
        className="modal-base edit-metadata-modal"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <h3>Edit Metadata</h3>

        {loading && <div className="edit-metadata-loading">Loading...</div>}
        {loadError && <p className="edit-metadata-error">{loadError}</p>}

        {!loading && !loadError && (
          <div className="edit-metadata-form">
            <label>
              <span>Type</span>
              <select value={mediaType} onChange={(e) => setMediaType(e.target.value)}>
                {MEDIA_TYPES.map((t) => (
                  <option key={t} value={t}>{t}</option>
                ))}
              </select>
            </label>

            <label className="grow">
              <span>Description</span>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </label>

            <label className="grow">
              <span>Extracted text</span>
              <textarea
                value={extractedText}
                onChange={(e) => setExtractedText(e.target.value)}
              />
            </label>

            <label>
              <span>Mentions</span>
              <input
                type="text"
                value={canonicalMentions}
                onChange={(e) => setCanonicalMentions(e.target.value)}
                placeholder="Comma-separated names"
              />
            </label>

            <label>
              <span>Location</span>
              <input
                type="text"
                value={locationText}
                onChange={(e) => setLocationText(e.target.value)}
                placeholder="e.g. New York, New York, US"
              />
            </label>

            <div className="modal-actions">
              <button type="button" onClick={onCancel}>Cancel</button>
              <button type="button" onClick={handleSave}>Save</button>
            </div>
          </div>
        )}
      </div>
    </ModalOverlay>
  );
}
