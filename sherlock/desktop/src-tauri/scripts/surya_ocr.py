#!/usr/bin/env python3
"""Minimal Surya OCR script for Frank Sherlock.

Usage: surya_ocr.py <image_path>
Output: JSON on stdout {"ok": true, "engine": "surya", "line_count": N, "text": "..."}

This script is intended to be run inside an isolated venv with surya-ocr installed.
"""
import json
import sys
from pathlib import Path


def main():
    if len(sys.argv) < 2:
        json.dump({"ok": False, "engine": "surya", "error": "no image path", "text": ""}, sys.stdout)
        return

    image_path = Path(sys.argv[1])
    if not image_path.exists():
        json.dump({"ok": False, "engine": "surya", "error": f"file not found: {image_path}", "text": ""}, sys.stdout)
        return

    try:
        from PIL import Image
        from surya.detection import DetectionPredictor
        from surya.recognition import RecognitionPredictor

        det_predictor = DetectionPredictor()
        rec_predictor = RecognitionPredictor()

        image = Image.open(image_path).convert("RGB")
        predictions = rec_predictor([image], [["en", "ja", "pt"]], det_predictor)

        lines = []
        if predictions and len(predictions) > 0:
            page = predictions[0]
            for line in page.text_lines:
                txt = (line.text or "").strip()
                if txt:
                    lines.append(txt)

        text = "\n".join(lines).strip()
        json.dump({"ok": True, "engine": "surya", "line_count": len(lines), "text": text}, sys.stdout)
    except Exception as e:
        json.dump({"ok": False, "engine": "surya", "error": str(e), "text": ""}, sys.stdout)


if __name__ == "__main__":
    main()
