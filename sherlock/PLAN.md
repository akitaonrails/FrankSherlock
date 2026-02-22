# Sherlock Main App Plan (from IDEA2 / THE MAIN APP)

## Goal
Build a local-first desktop app to scan NAS folders read-only, classify images with local AI, index results, and support natural-language retrieval with thumbnail grid + quick preview.

## Locked Requirements
- Media scope for v1: images only.
- Extensibility: architecture must support adding audio/video later without schema breakage.
- OCR priority: `pt-BR` and `en` first; keep pipeline multilingual-ready.
- Privacy: strictly local inference/indexing (Ollama/local models only, no cloud fallback).
- Packaging: Linux AppImage is a hard requirement for v1.
- Operational robustness: avoid stale Ollama models lingering in memory.
- Query UX must stay responsive even for very broad searches (thousands of matches).

## Proposed Stack (v1)
- Desktop shell: Tauri (chosen; Linux-first, lightweight, AppImage-friendly).
- UI: React + Vite (single search view + thumbnail grid + preview pane).
- Core service: Python worker (reuse current classification/OCR logic and Ollama integration).
- Local DB: SQLite (metadata + FTS5) + sqlite-vec (embeddings) in `~/.local/share/frank_sherlock/db`.
- Embeddings: local model via Ollama embeddings endpoint.

## Directory Layout
- `sherlock/desktop/` Tauri + frontend.
- `sherlock/backend/` Python service/worker and scan pipeline.
- `sherlock/shared/` JSON schemas for result/index payloads.

Runtime user data (outside repo):
- `~/.local/share/frank_sherlock/db/index.sqlite`
- `~/.local/share/frank_sherlock/cache/classifications/<scan-id>/...` (mirrored path outputs)
- `~/.local/share/frank_sherlock/cache/thumbnails/...`
- `~/.local/share/frank_sherlock/cache/scans/...` (scan manifests/fingerprints)

## Core Design

### 1) Read-only scanning
- Never write to target roots.
- Track each source root by stable id: `(device, inode of root when available, canonical path)`.
- Incremental detection:
  - fast pass: directory mtimes + file size + file mtime.
  - fingerprint only when needed: partial hash (head+tail blocks) for large files, full hash for small files.
- Rename/move detection:
  - if fingerprint unchanged but path changed, update mapping without reclassification.

### 2) Classification pipeline
- Job queue, single inference worker per GPU-heavy model.
- Reuse classification outputs already tested (`.yml` + `.txt` mirrored structure).
- Skip unchanged files using fingerprint + model-version cache keys.
- Separate OCR pass only for document-like/text-heavy images.
- Store language hints per file (`pt`, `en`, `mixed`, `unknown`) to support future multilingual routing.

### 3) Indexing and retrieval
- Persist file metadata: path, media type, timestamps, size, fingerprint, confidence, extracted fields.
- Persist extracted text and structured JSON.
- Build two query paths:
  - keyword/filters (FTS + metadata range filters).
  - semantic (vector similarity over extracted text/summary).
- Rank by blended score:
  - semantic score + FTS score + confidence weight + freshness hints.
- Use one global index with `root_id` partitioning for simplicity and cross-root search.
- Always keep `root_id` filterable to emulate per-root isolated queries cheaply.

### 3.1 Natural-language query schema (local, simple)
- Parse user text into:
  - `query_text` (free text for FTS/vector),
  - `media_types[]`,
  - `date_from/date_to`,
  - `min_confidence`,
  - `root_scope[]`.
- Implementation strategy:
  - deterministic parser for obvious patterns (`from 2024`, `receipts`, `anime girl`, `in Dropbox`),
  - optional local small LLM pass only when parser confidence is low.

### 4) Desktop UX (v1)
- Command: `sherlock /path/to/root` starts/updates scan for that root.
- App view:
  - query box (natural language).
  - filter chips (media type, date range, min confidence).
  - thumbnail grid with lazy loading.
  - quick preview pane; space key opens full preview modal.
- Background indexing progress + resumable jobs.

### 4.2 Query responsiveness safeguards
- Never render unbounded result sets in one pass.
- Use paginated/virtualized grid rendering.
- API returns a capped page (`limit/offset` or cursor) plus total-count estimate.
- Run queries asynchronously with cancellation/debounce on new keystrokes.
- Progressive hydration:
  - return metadata first,
  - load thumbnails lazily as tiles enter viewport.
- Enforce query time budget and fallback strategy:
  - fast metadata/FTS first,
  - semantic rerank only on top-K candidates.

### 4.1 Ollama lifecycle control
- Default inference calls use bounded keep-alive.
- On scan completion or idle timeout, explicitly unload model from Ollama.
- Maintain a single model lease in backend so only one heavy model stays loaded.
- If process crashes, startup cleanup checks active Ollama models and releases stale ones.

## Desktop Choice: Tauri vs Electron
- Tauri advantages:
  - lower RAM/CPU overhead for NAS-heavy background workflows;
  - smaller bundle size, faster AppImage distribution;
  - Rust side is good for filesystem-sensitive operations and long-running workers.
- Tauri risks:
  - steeper integration for some JS desktop APIs;
  - Rust + Python orchestration needs stricter process supervision.
- Electron advantages:
  - fastest path for rich JS desktop UI and ecosystem plugins;
  - simpler all-JS frontend development/debugging.
- Electron risks:
  - heavier memory footprint and larger AppImage artifacts;
  - less attractive for always-on indexing on resource-constrained machines.
- Recommendation for this project: Tauri, because your priorities are local robustness, low overhead, and AppImage-first delivery.

## Global vs Distributed Index
- Global index advantages:
  - one query pipeline, easier ranking across all roots,
  - simpler UX (`search everything` by default).
- Global index risks:
  - larger DB growth and vacuum/reindex management complexity over time.
- Per-root index advantages:
  - strong isolation, easier archival/repair per root.
- Per-root index risks:
  - harder cross-root search and ranking; federated query complexity.
- Chosen: global SQLite with `root_id` partitioning + per-root maintenance commands.

## Implementation Phases
- Phase A: Bootstrap project skeleton under `sherlock/` (Tauri + Python backend contracts).
- Phase B: Scanner + manifest cache + incremental/rename detection.
- Phase C: Classification worker integration (reuse current best models and prompts).
- Phase D: SQLite schema + FTS + vector index + retrieval API.
- Phase E: Desktop UI (search, grid, preview, progress).
- Phase F: Packaging (Linux AppImage) + first end-to-end run docs.

## Acceptance Criteria (v1)
- Can scan one large NAS path and resume without rescanning unchanged files.
- Detects rename/move without re-running AI when fingerprint is unchanged.
- Query by natural language returns relevant files with confidence-aware ranking.
- Grid view and space-preview work on indexed results.
- All writes happen only under `~/.local/share/frank_sherlock/`.
- Image-only pipeline is stable, with extensibility points for audio/video providers.
- AppImage build/install flow is documented and reproducible.
- Ollama model memory is released automatically after scan/idle completion.

## Decision Status
1. Desktop shell: Tauri.
2. Index strategy: global DB with `root_id` partitioning.
