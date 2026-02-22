# Sherlock (Main App Prototype)

Tauri desktop app + Rust indexing core for local-only image search.

## Scope (v1)
- Images only.
- Global SQLite database in `~/.local/share/frank_sherlock/db/index.sqlite`.
- Read-only scanning of target roots.
- Natural query parsing + paginated result retrieval.
- Responsive result browsing for broad matches.

## Structure
- `desktop/`: Tauri app (Rust backend commands + React frontend).
- `backend/`: reserved for future Python inference worker integration.
- `shared/`: reserved for shared schemas/contracts.

## Run (Arch Linux / Hyprland / Wayland)
From `sherlock/desktop`:

```bash
npm install
npm run tauri:dev
```

If WebKit rendering has issues on NVIDIA/Wayland:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=wayland,x11 npm run tauri:dev
```

## Build AppImage
From `sherlock/desktop`:

```bash
npm run tauri:build
```

Output AppImage will be under `desktop/src-tauri/target/release/bundle/appimage/`.
Note: first AppImage bundle run downloads Tauri AppImage helper binaries (network required once).

## Test
From `sherlock/desktop`:

```bash
npm run test:rust
```

This runs unit tests for:
- path/config resolution,
- natural query parser,
- SQLite schema + pagination + FTS search,
- scanner rename/move detection.
