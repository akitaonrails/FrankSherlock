Based on the results in `docs/RESULTS.md`, I need a new prototype:

- New `classification` subdirectory with a short Python prototype using whichever image classification model won the benchmarks.
- Run it on all images in `test_files` and write results into a `test_results` directory mirroring the source structure. So `old_image/a.jpg` becomes `old_image/a.[yml,md,txt]` — whatever format is easiest to review and later index.
- The model probably takes up most of the GPU when loaded, so I'm guessing we can't parallelize. Just confirming.
- Go with the best strategy from the benchmarks. Squeeze out the best classification we can.
- If an image has a "girl character," I want the full thing: "Ranma from the Ranma 1/2 series." If it's a bank receipt, I want dates, amounts, transaction IDs — accurate enough to search later, not just filenames and mtime.

## The main app

- Build on everything from the research.
- Desktop app, file-explorer-style, with a thumbnail grid.
- Natural language search over the index (using an LLM).
- First launch will look something like `sherlock /mnt/terachad/Dropbox` from the console.
- Centralized user database at `~/.local/share/frank_sherlock/db`.
- That local directory stores classification files (mirroring the scanned file paths) and the search index.
- The target directory (e.g., Dropbox) is read-only. Never overwrite, delete, or modify anything in it.
- NAS directories can be very deep. A naive recursive find might be slow, so cache scan results and only look for files added or modified since the last scan.
- Detect renamed/moved directories and don't re-scan the same files. A fast file fingerprint should work.
- Classification output feeds a search index. I want to query by name, date range, and content, with confidence scores for ranking.
- Version one can be simple: search bar, thumbnail grid, results.
- Thumbnail generation is expensive, so cache them under `.local/share`.
- Works with any NAS directory. Cached scans, skip re-scanning when nothing changed.
- Quick preview on a result (spacebar or similar).
- No file manipulation (move, rename) in v1. That's later.
- Unsure about Electron vs. Rust Tauri.
- Assume Ollama is installed and running.
- Keep it as self-contained as possible. Ideally an AppImage on Linux.
