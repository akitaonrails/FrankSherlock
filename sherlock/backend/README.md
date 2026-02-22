# Backend Worker (Planned)

This directory is reserved for the Python inference worker that will:
- consume scan jobs from the Rust/Tauri layer,
- run image classification + OCR using local Ollama and OCR engines,
- write classification artifacts to `~/.local/share/frank_sherlock/cache/classifications/`,
- push extracted content into the global SQLite index.

Current Tauri prototype already handles:
- global DB bootstrap,
- incremental read-only scanning with move detection,
- natural query parsing and paginated retrieval.
