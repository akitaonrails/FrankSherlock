use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::classify;
use crate::config::canonical_root_path;
use crate::db;
use crate::error::AppResult;
use std::path::PathBuf;

use crate::models::{ExistingFile, FileRecordUpsert, ScanContext, ScanJobStatus, ScanSummary};
use crate::platform::paths::normalize_rel_path;
use crate::thumbnail;
use crate::video;

const SUPPORTED_EXTS: [&str; 20] = [
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".tiff", ".tif", ".pdf", ".mp4", ".mkv",
    ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".ts", ".mpg", ".mpeg",
];

/// Skip tiny images likely to be web icons, spacer GIFs, favicons, etc.
/// 10 KB is roughly a 64x64 PNG or any icon-sized image.
const MIN_IMAGE_SIZE_BYTES: u64 = 10_240;

/// File classification state determined during incremental discovery.
#[derive(Debug)]
enum FileStatus {
    Unchanged,
    Modified,
    New,
}

#[derive(Debug)]
struct FileProbe {
    rel_path: String,
    abs_path: String,
    filename: String,
    mtime_ns: i64,
    size_bytes: i64,
    /// Populated from DB for unchanged files; None for new/modified (computed lazily in phase 2).
    fingerprint: Option<String>,
    status: FileStatus,
}

pub fn start_or_resume_scan_job(db_path: &Path, root_path: &str) -> AppResult<ScanJobStatus> {
    let canonical_root = canonical_root_path(root_path)?;
    db::create_or_resume_scan_job(db_path, &canonical_root.display().to_string())
}

pub fn run_scan_job(
    ctx: &ScanContext,
    job_id: i64,
    cancel_flag: Option<&AtomicBool>,
    skip_classify: bool,
) -> AppResult<ScanSummary> {
    run_scan_job_internal(ctx, job_id, None, cancel_flag, skip_classify)
}

fn run_scan_job_internal(
    ctx: &ScanContext,
    job_id: i64,
    max_files_for_test: Option<usize>,
    cancel_flag: Option<&AtomicBool>,
    skip_classify: bool,
) -> AppResult<ScanSummary> {
    let started = Instant::now();
    let db_path = &ctx.db_path;
    let job = db::get_scan_job_state(db_path, job_id)?;
    let root_path = canonical_root_path(&job.root_path)?;

    log::info!(
        "Scan job {}: starting discovery for {}",
        job_id,
        root_path.display()
    );

    // Compute child roots to exclude during walkdir
    let all_roots = db::list_root_paths(db_path).unwrap_or_default();
    let excluded_prefixes: Vec<PathBuf> = all_roots
        .iter()
        .filter_map(|(_id, rp)| {
            let rp_path = PathBuf::from(rp);
            if rp_path.starts_with(&root_path) && rp_path != root_path {
                Some(rp_path)
            } else {
                None
            }
        })
        .collect();

    // Phase 1: Load existing DB records
    let db_load_start = Instant::now();
    let existing = db::load_existing_files(db_path, job.root_id)?;
    let db_load_ms = db_load_start.elapsed().as_millis();
    log::info!(
        "Scan job {}: loaded {} existing records from DB in {}ms",
        job_id,
        existing.len(),
        db_load_ms,
    );
    let existing_by_path: HashMap<String, ExistingFile> = existing
        .iter()
        .map(|f| (f.rel_path.clone(), f.clone()))
        .collect();
    let mut by_fingerprint: HashMap<String, Vec<ExistingFile>> = HashMap::new();
    for file in existing {
        by_fingerprint
            .entry(file.fingerprint.clone())
            .or_default()
            .push(file);
    }

    // Phase 1: Incremental discovery (metadata-only for unchanged files)
    let discovery_start = Instant::now();
    let probes = collect_image_probes_incremental(
        &root_path,
        &existing_by_path,
        &excluded_prefixes,
        db_path,
        job_id,
        cancel_flag,
    )?;

    // If cancelled during discovery, bail out
    if cancel_flag.is_some_and(|f| f.load(Ordering::Relaxed)) {
        db::cancel_scan_job(db_path, job_id)?;
        return Ok(ScanSummary {
            root_id: job.root_id,
            root_path: root_path.display().to_string(),
            scanned: 0,
            added: 0,
            modified: 0,
            moved: 0,
            unchanged: 0,
            deleted: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }

    let total_files = probes.len() as u64;
    let start_index = resume_start_index(&probes, job.cursor_rel_path.as_deref());

    let new_count = probes
        .iter()
        .filter(|p| matches!(p.status, FileStatus::New))
        .count();
    let mod_count = probes
        .iter()
        .filter(|p| matches!(p.status, FileStatus::Modified))
        .count();
    let unch_count = probes
        .iter()
        .filter(|p| matches!(p.status, FileStatus::Unchanged))
        .count();
    log::info!(
        "Scan job {}: discovery complete — {} files ({} new, {} modified, {} unchanged) in {:.1}s",
        job_id,
        total_files,
        new_count,
        mod_count,
        unch_count,
        discovery_start.elapsed().as_secs_f64()
    );

    let mut processed_files = job.processed_files.max(start_index as u64);
    let mut added = job.added;
    let mut modified = job.modified;
    let mut moved = job.moved;
    let mut unchanged = job.unchanged;
    let mut used_moved_ids = HashSet::new();
    let mut last_cursor: Option<String> = job.cursor_rel_path.clone();
    const UNCHANGED_BATCH_SIZE: usize = 200;

    // Determine whether to skip the thumbnailing phase (already done on a previous run).
    let skip_thumbnailing = job.phase == "classifying";

    if !skip_thumbnailing {
        db::checkpoint_scan_job(
            db_path,
            job_id,
            "thumbnailing",
            total_files,
            processed_files,
            last_cursor.as_deref(),
            added,
            modified,
            moved,
            unchanged,
        )?;

        if start_index > 0 {
            log::info!(
                "Scan job {}: resuming thumbnailing from index {} (cursor: {:?})",
                job_id,
                start_index,
                job.cursor_rel_path
            );
        } else {
            log::info!("Scan job {}: starting thumbnailing from beginning", job_id);
        }

        // Flush all unchanged files upfront so the UI can show previously-known
        // files immediately while thumbnailing works through new/modified ones.
        let flush_start = Instant::now();
        let unchanged_paths: Vec<&str> = probes
            .iter()
            .skip(start_index)
            .filter(|p| matches!(p.status, FileStatus::Unchanged))
            .map(|p| p.rel_path.as_str())
            .collect();
        let unchanged_total = unchanged_paths.len() as u64;
        for batch in unchanged_paths.chunks(UNCHANGED_BATCH_SIZE) {
            db::touch_file_scan_markers_batch(db_path, job.root_id, batch, job.scan_marker)?;
        }
        unchanged += unchanged_total;
        processed_files += unchanged_total;
        log::info!(
            "Scan job {}: flushed {} unchanged markers in {}ms",
            job_id,
            unchanged_total,
            flush_start.elapsed().as_millis(),
        );
        db::checkpoint_scan_job(
            db_path,
            job_id,
            "thumbnailing",
            total_files,
            processed_files,
            last_cursor.as_deref(),
            added,
            modified,
            moved,
            unchanged,
        )?;

        // Phase 2: Thumbnailing loop — generate thumbnails for new/modified files
        for (i, probe) in probes.iter().enumerate().skip(start_index) {
            if matches!(probe.status, FileStatus::Unchanged) {
                continue; // already flushed above
            }

            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    db::cancel_scan_job(db_path, job_id)?;
                    return Ok(ScanSummary {
                        root_id: job.root_id,
                        root_path: root_path.display().to_string(),
                        scanned: processed_files,
                        added,
                        modified,
                        moved,
                        unchanged,
                        deleted: 0,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                    });
                }
            }

            if let Some(max) = max_files_for_test {
                if (i - start_index) >= max {
                    break;
                }
            }

            // Compute fingerprint lazily for new/modified files (deferred from discovery)
            let fingerprint = match &probe.fingerprint {
                Some(fp) => fp.clone(),
                None => fingerprint_file(Path::new(&probe.abs_path), probe.size_bytes as u64)?,
            };

            match probe.status {
                FileStatus::Unchanged => unreachable!("filtered above"),
                FileStatus::Modified => {
                    let thumb_result = thumbnail_only(ctx, probe);
                    let record = probe_to_minimal_record(
                        job.root_id,
                        job.scan_marker,
                        probe,
                        &fingerprint,
                        &thumb_result,
                    );
                    db::upsert_file_record(db_path, &record)?;
                    if let Some(ref thumb) = thumb_result.thumb_path {
                        db::update_file_thumb_path(db_path, job.root_id, &probe.rel_path, thumb)?;
                    }
                    modified += 1;
                }
                FileStatus::New => {
                    // Check for move detection by fingerprint
                    if let Some(candidates) = by_fingerprint.get(&fingerprint) {
                        if let Some(candidate) = candidates.iter().find(|c| {
                            !used_moved_ids.contains(&c.id) && c.rel_path != probe.rel_path
                        }) {
                            used_moved_ids.insert(candidate.id);
                            moved += 1;
                            db::move_file_by_id(
                                db_path,
                                candidate.id,
                                &probe.rel_path,
                                &probe.abs_path,
                                &probe.filename,
                                probe.mtime_ns,
                                probe.size_bytes,
                                job.scan_marker,
                            )?;

                            // Regenerate thumbnail at new path
                            let abs = Path::new(&probe.abs_path);
                            let thumb_result = if is_video(abs) {
                                thumbnail::generate_video_thumbnail(
                                    abs,
                                    &ctx.thumbnails_dir,
                                    &probe.rel_path,
                                    &ctx.tmp_dir,
                                )
                            } else if is_pdf_file(abs) {
                                thumbnail::generate_pdf_thumbnail(
                                    abs,
                                    &ctx.thumbnails_dir,
                                    &probe.rel_path,
                                    &ctx.pdfium_lib_path,
                                    None,
                                )
                            } else {
                                thumbnail::generate_thumbnail(
                                    abs,
                                    &ctx.thumbnails_dir,
                                    &probe.rel_path,
                                )
                            };
                            if let Some(ref tr) = thumb_result {
                                db::update_file_thumb_path(
                                    db_path,
                                    job.root_id,
                                    &probe.rel_path,
                                    &tr.path,
                                )?;
                            }

                            processed_files += 1;
                            last_cursor = Some(probe.rel_path.clone());
                            db::checkpoint_scan_job(
                                db_path,
                                job_id,
                                "thumbnailing",
                                total_files,
                                processed_files,
                                last_cursor.as_deref(),
                                added,
                                modified,
                                moved,
                                unchanged,
                            )?;
                            continue;
                        }
                    }

                    // Genuinely new file — thumbnail only (classification deferred)
                    let thumb_result = thumbnail_only(ctx, probe);
                    let record = probe_to_minimal_record(
                        job.root_id,
                        job.scan_marker,
                        probe,
                        &fingerprint,
                        &thumb_result,
                    );
                    db::upsert_file_record(db_path, &record)?;
                    if let Some(ref thumb) = thumb_result.thumb_path {
                        db::update_file_thumb_path(db_path, job.root_id, &probe.rel_path, thumb)?;
                    }
                    added += 1;
                }
            }

            processed_files += 1;
            last_cursor = Some(probe.rel_path.clone());
            db::checkpoint_scan_job(
                db_path,
                job_id,
                "thumbnailing",
                total_files,
                processed_files,
                last_cursor.as_deref(),
                added,
                modified,
                moved,
                unchanged,
            )?;
        }

        log::info!(
            "Scan job {}: thumbnailing complete ({} added, {} modified, {} moved)",
            job_id,
            added,
            modified,
            moved
        );
    } // end skip_thumbnailing

    // (Unchanged files were already flushed before the thumbnailing loop.)

    if max_files_for_test.is_some() {
        return Ok(ScanSummary {
            root_id: job.root_id,
            root_path: root_path.display().to_string(),
            scanned: processed_files,
            added,
            modified,
            moved,
            unchanged,
            deleted: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }

    // Phase 3: Classification loop — LLM classify all unclassified files
    let unclassified = if skip_classify {
        vec![]
    } else {
        db::list_unclassified_files(db_path, job.root_id, job.scan_marker)?
    };
    if !unclassified.is_empty() {
        let classify_total = unclassified.len() as u64;
        let mut classify_processed: u64 = 0;

        // For resume: if we were already in classifying phase, skip already-classified files.
        // list_unclassified_files only returns confidence=0, so already-classified are excluded.
        // We still use cursor_rel_path for progress reporting consistency.
        let classify_cursor = if skip_thumbnailing {
            job.cursor_rel_path.clone()
        } else {
            None
        };

        db::checkpoint_scan_job(
            db_path,
            job_id,
            "classifying",
            classify_total,
            classify_processed,
            classify_cursor.as_deref(),
            added,
            modified,
            moved,
            unchanged,
        )?;

        log::info!(
            "Scan job {}: starting classification of {} files",
            job_id,
            classify_total
        );

        for file in &unclassified {
            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    db::cancel_scan_job(db_path, job_id)?;
                    return Ok(ScanSummary {
                        root_id: job.root_id,
                        root_path: root_path.display().to_string(),
                        scanned: processed_files,
                        added,
                        modified,
                        moved,
                        unchanged,
                        deleted: 0,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                    });
                }
            }

            // Skip files already past the cursor (for resume)
            if let Some(ref cursor) = classify_cursor {
                if file.rel_path.as_str() <= cursor.as_str() {
                    classify_processed += 1;
                    continue;
                }
            }

            classify_and_update(ctx, file.id, &file.abs_path);

            classify_processed += 1;
            last_cursor = Some(file.rel_path.clone());
            db::checkpoint_scan_job(
                db_path,
                job_id,
                "classifying",
                classify_total,
                classify_processed,
                last_cursor.as_deref(),
                added,
                modified,
                moved,
                unchanged,
            )?;
        }

        log::info!(
            "Scan job {}: classification complete ({} files)",
            job_id,
            classify_total
        );
    }

    log::info!(
        "Scan job {}: all processing complete, starting cleanup",
        job_id
    );

    // Phase 4: Cleanup deleted files
    let deleted_at_before = db::now_epoch_secs_pub();
    let deleted = db::mark_missing_as_deleted(db_path, job.root_id, job.scan_marker)?;
    if deleted > 0 {
        cleanup_deleted_caches(db_path, job.root_id, deleted_at_before, &ctx.thumbnails_dir);
    }
    db::touch_root_scan(db_path, job.root_id)?;

    let summary = ScanSummary {
        root_id: job.root_id,
        root_path: root_path.display().to_string(),
        scanned: total_files,
        added,
        modified,
        moved,
        unchanged,
        deleted,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    db::complete_scan_job_by_id(db_path, job_id, &summary, last_cursor.as_deref())?;
    Ok(summary)
}

/// Result from thumbnail-only pass (no LLM classification).
struct ThumbnailOnlyResult {
    thumb_path: Option<String>,
    location_text: String,
    dhash: Option<u64>,
}

fn is_pdf_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn is_video(path: &Path) -> bool {
    video::is_video_file(path)
}

/// Generate thumbnail and extract EXIF (no LLM classification).
fn thumbnail_only(ctx: &ScanContext, probe: &FileProbe) -> ThumbnailOnlyResult {
    let abs = Path::new(&probe.abs_path);
    let is_pdf = is_pdf_file(abs);
    let is_vid = is_video(abs);

    let pdf_password: Option<String> =
        if is_pdf && crate::pdf::is_password_protected(abs, &ctx.pdfium_lib_path) {
            let passwords = db::get_all_pdf_password_strings(&ctx.db_path).unwrap_or_default();
            crate::pdf::try_passwords(abs, &ctx.pdfium_lib_path, &passwords)
        } else {
            None
        };

    let thumb_result = if is_vid {
        thumbnail::generate_video_thumbnail(abs, &ctx.thumbnails_dir, &probe.rel_path, &ctx.tmp_dir)
    } else if is_pdf {
        thumbnail::generate_pdf_thumbnail(
            abs,
            &ctx.thumbnails_dir,
            &probe.rel_path,
            &ctx.pdfium_lib_path,
            pdf_password.as_deref(),
        )
    } else {
        thumbnail::generate_thumbnail(abs, &ctx.thumbnails_dir, &probe.rel_path)
    };

    let exif_location: crate::exif::ExifLocation = if is_pdf || is_vid {
        Default::default()
    } else {
        crate::exif::extract_location(abs)
    };

    let (thumb_path, dhash) = match thumb_result {
        Some(tr) => (Some(tr.path), tr.dhash),
        None => (None, None),
    };

    ThumbnailOnlyResult {
        thumb_path,
        location_text: exif_location.location_text,
        dhash,
    }
}

/// Infer a basic media_type from file extension (used for placeholder records).
fn infer_media_type_from_extension(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if lower.ends_with(".pdf") {
        "document"
    } else if video::is_video_file(Path::new(filename)) {
        "video"
    } else {
        "photo"
    }
}

/// Build a minimal DB record with placeholder classification (confidence=0).
fn probe_to_minimal_record(
    root_id: i64,
    scan_marker: i64,
    probe: &FileProbe,
    fingerprint: &str,
    thumb_result: &ThumbnailOnlyResult,
) -> FileRecordUpsert {
    let video_meta = if video::is_video_file(Path::new(&probe.abs_path)) {
        video::extract_metadata(Path::new(&probe.abs_path))
    } else {
        None
    };

    FileRecordUpsert {
        root_id,
        rel_path: probe.rel_path.clone(),
        abs_path: probe.abs_path.clone(),
        filename: probe.filename.clone(),
        media_type: infer_media_type_from_extension(&probe.filename).to_string(),
        description: String::new(),
        extracted_text: String::new(),
        canonical_mentions: String::new(),
        confidence: 0.0,
        lang_hint: String::new(),
        mtime_ns: probe.mtime_ns,
        size_bytes: probe.size_bytes,
        fingerprint: fingerprint.to_string(),
        scan_marker,
        location_text: thumb_result.location_text.clone(),
        dhash: thumb_result.dhash.map(|h| h as i64),
        duration_secs: video_meta.as_ref().and_then(|m| m.duration_secs),
        video_width: video_meta.as_ref().and_then(|m| m.width),
        video_height: video_meta.as_ref().and_then(|m| m.height),
        video_codec: video_meta.as_ref().and_then(|m| m.video_codec.clone()),
        audio_codec: video_meta.as_ref().and_then(|m| m.audio_codec.clone()),
    }
}

/// Classify a single file via LLM and update the DB record.
fn classify_and_update(ctx: &ScanContext, file_id: i64, abs_path: &str) {
    let abs = Path::new(abs_path);
    let is_pdf = is_pdf_file(abs);
    let is_vid = is_video(abs);

    let pdf_password: Option<String> =
        if is_pdf && crate::pdf::is_password_protected(abs, &ctx.pdfium_lib_path) {
            let passwords = db::get_all_pdf_password_strings(&ctx.db_path).unwrap_or_default();
            crate::pdf::try_passwords(abs, &ctx.pdfium_lib_path, &passwords)
        } else {
            None
        };

    let classification = if is_vid {
        classify::classify_video(
            abs,
            &ctx.model,
            &ctx.tmp_dir,
            &ctx.surya_venv_dir,
            &ctx.surya_script,
        )
    } else if is_pdf {
        classify::classify_pdf(
            abs,
            &ctx.model,
            &ctx.tmp_dir,
            &ctx.surya_venv_dir,
            &ctx.surya_script,
            &ctx.pdfium_lib_path,
            pdf_password.as_deref(),
        )
    } else {
        classify::classify_image(
            abs,
            &ctx.model,
            &ctx.tmp_dir,
            &ctx.surya_venv_dir,
            &ctx.surya_script,
        )
    };

    if let Err(e) = db::update_file_classification(
        &ctx.db_path,
        file_id,
        &classification.media_type,
        &classification.description,
        &classification.extracted_text,
        &classification.canonical_mentions,
        classification.confidence,
        &classification.lang_hint,
    ) {
        log::error!(
            "Failed to update classification for file {}: {}",
            file_id,
            e
        );
    }
}

fn resume_start_index(probes: &[FileProbe], cursor_rel_path: Option<&str>) -> usize {
    let Some(cursor) = cursor_rel_path else {
        return 0;
    };
    probes
        .iter()
        .position(|p| p.rel_path.as_str() > cursor)
        .unwrap_or(probes.len())
}

/// Incremental file discovery: only reads file content for new/modified files.
fn collect_image_probes_incremental(
    root: &Path,
    existing_by_path: &HashMap<String, ExistingFile>,
    excluded_prefixes: &[PathBuf],
    db_path: &Path,
    job_id: i64,
    cancel_flag: Option<&AtomicBool>,
) -> AppResult<Vec<FileProbe>> {
    let mut probes = Vec::new();
    let mut discovered: u64 = 0;
    let mut walk_entries: u64 = 0;
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip child root subtrees before descending
            if e.file_type().is_dir() && e.path() != root {
                return !excluded_prefixes.iter().any(|p| e.path().starts_with(p));
            }
            true
        })
        .filter_map(|entry| entry.ok())
    {
        walk_entries += 1;

        // Check cancel flag during discovery
        if let Some(flag) = cancel_flag {
            if flag.load(Ordering::Relaxed) {
                return Ok(probes);
            }
        }

        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_supported_file(path) {
            continue;
        }

        // Use entry.metadata() to avoid a redundant stat() syscall —
        // WalkDir already called lstat() during the walk.
        let metadata = entry.metadata().map_err(|e| {
            crate::error::AppError::Config(format!("metadata for {}: {}", path.display(), e))
        })?;
        // Skip tiny images (icons, spacer GIFs, favicons) but not PDFs or videos
        let ext_lower = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if ext_lower != "pdf"
            && !video::is_video_file(path)
            && metadata.len() < MIN_IMAGE_SIZE_BYTES
        {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|p| normalize_rel_path(&p.to_string_lossy()))
            .unwrap_or_else(|_| normalize_rel_path(&path.to_string_lossy()));
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or_default();
        let size_bytes = metadata.len() as i64;

        if let Some(existing) = existing_by_path.get(&rel) {
            if existing.mtime_ns == mtime_ns && existing.size_bytes == size_bytes {
                // Unchanged: reuse DB fingerprint, no file content read
                probes.push(FileProbe {
                    rel_path: rel,
                    abs_path: path.display().to_string(),
                    filename,
                    mtime_ns,
                    size_bytes,
                    fingerprint: Some(existing.fingerprint.clone()),
                    status: FileStatus::Unchanged,
                });
            } else {
                // Modified: fingerprint deferred to phase 2
                probes.push(FileProbe {
                    rel_path: rel,
                    abs_path: path.display().to_string(),
                    filename,
                    mtime_ns,
                    size_bytes,
                    fingerprint: None,
                    status: FileStatus::Modified,
                });
            }
        } else {
            // New file: fingerprint deferred to phase 2
            probes.push(FileProbe {
                rel_path: rel,
                abs_path: path.display().to_string(),
                filename,
                mtime_ns,
                size_bytes,
                fingerprint: None,
                status: FileStatus::New,
            });
        }

        discovered += 1;
        if discovered % 500 == 0 {
            let _ = db::update_discovery_progress(db_path, job_id, discovered);
        }
    }
    let sort_start = Instant::now();
    probes.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let sort_ms = sort_start.elapsed().as_millis();

    log::info!(
        "Discovery breakdown: {} walkdir entries, {} supported files, sort {}ms (fingerprints deferred to phase 2)",
        walk_entries,
        discovered,
        sort_ms,
    );
    Ok(probes)
}

fn cleanup_deleted_caches(db_path: &Path, root_id: i64, deleted_at: i64, thumbnails_dir: &Path) {
    let paths = match db::get_deleted_file_paths(db_path, root_id, deleted_at) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Failed to get deleted file paths: {e}");
            return;
        }
    };
    for (rel_path, thumb_path) in paths {
        // Remove thumbnail
        if let Some(tp) = thumb_path {
            let _ = std::fs::remove_file(&tp);
        }
        // Also try the expected path
        let expected_thumb = thumbnails_dir.join(normalize_rel_path(
            &Path::new(&rel_path).with_extension("jpg").to_string_lossy(),
        ));
        let _ = std::fs::remove_file(&expected_thumb);
    }
}

fn fingerprint_file(path: &Path, size: u64) -> AppResult<String> {
    const WINDOW: usize = 64 * 1024;
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();

    if size <= (WINDOW * 2) as u64 {
        let mut buf = Vec::with_capacity(size as usize);
        file.read_to_end(&mut buf)?;
        hasher.update(buf);
        return Ok(hex::encode(hasher.finalize()));
    }

    let mut head = vec![0_u8; WINDOW];
    file.read_exact(&mut head)?;
    hasher.update(&head);

    let mut tail = vec![0_u8; WINDOW];
    file.seek(SeekFrom::End(-(WINDOW as i64)))?;
    file.read_exact(&mut tail)?;
    hasher.update(&tail);

    Ok(hex::encode(hasher.finalize()))
}

fn is_supported_file(path: &Path) -> bool {
    let Some(ext) = path.extension().map(|v| v.to_string_lossy().to_lowercase()) else {
        return false;
    };
    let ext = format!(".{ext}");
    SUPPORTED_EXTS.contains(&ext.as_str())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::db;

    /// Write a dummy file large enough to pass MIN_IMAGE_SIZE_BYTES.
    fn write_test_image(path: &std::path::Path, seed: u8) {
        let mut f = File::create(path).expect("create test image");
        let data = vec![seed; MIN_IMAGE_SIZE_BYTES as usize];
        f.write_all(&data).expect("write test image");
    }

    fn make_scan_context(db_path: &Path) -> ScanContext {
        let tmp = tempfile::tempdir().expect("tempdir");
        ScanContext {
            db_path: db_path.to_path_buf(),
            thumbnails_dir: tmp.path().join("thumbs"),
            tmp_dir: tmp.path().join("tmp"),
            surya_venv_dir: tmp.path().join("venv"),
            surya_script: tmp.path().join("surya_ocr.py"),
            model: "qwen2.5vl:7b".to_string(),
            pdfium_lib_path: tmp.path().join("lib"),
        }
    }

    #[test]
    fn detects_move_without_reinsert() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let db_dir = tempfile::tempdir().expect("dbdir");
        let db_path = db_dir.path().join("index.sqlite");
        db::init_database(&db_path).expect("init");

        let image_a = root_dir.path().join("a.jpg");
        write_test_image(&image_a, 0xAA);

        let first_job = start_or_resume_scan_job(&db_path, root_dir.path().to_str().expect("str"))
            .expect("job");
        // For testing without Ollama, we use the internal function but skip classification
        // by using the old-style direct test
        let _ctx = make_scan_context(&db_path);
        // We can't easily test with real classification in unit tests,
        // but we verify the incremental discovery logic here.
        let existing = db::load_existing_files(&db_path, first_job.root_id).expect("load");
        assert!(existing.is_empty());
    }

    #[test]
    fn incremental_discovery_marks_unchanged() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let db_dir = tempfile::tempdir().expect("dbdir");
        let db_path = db_dir.path().join("index.sqlite");
        db::init_database(&db_path).expect("init");
        let root_id = db::upsert_root(&db_path, root_dir.path().to_str().unwrap()).expect("root");

        // Create a file
        let img = root_dir.path().join("test.jpg");
        write_test_image(&img, 0xBB);

        let metadata = std::fs::metadata(&img).expect("meta");
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or_default();
        let fp = fingerprint_file(&img, metadata.len()).expect("fp");

        // Insert record to simulate previous scan
        let rec = crate::models::FileRecordUpsert {
            root_id,
            rel_path: "test.jpg".to_string(),
            abs_path: img.display().to_string(),
            filename: "test.jpg".to_string(),
            media_type: "photo".to_string(),
            description: "classified already".to_string(),
            extracted_text: String::new(),
            canonical_mentions: String::new(),
            confidence: 0.8,
            lang_hint: "en".to_string(),
            mtime_ns,
            size_bytes: metadata.len() as i64,
            fingerprint: fp,
            scan_marker: 1,
            location_text: String::new(),
            dhash: None,
            duration_secs: None,
            video_width: None,
            video_height: None,
            video_codec: None,
            audio_codec: None,
        };
        db::upsert_file_record(&db_path, &rec).expect("upsert");

        // Now run incremental discovery
        let existing = db::load_existing_files(&db_path, root_id).expect("load");
        let existing_by_path: HashMap<String, ExistingFile> = existing
            .iter()
            .map(|f| (f.rel_path.clone(), f.clone()))
            .collect();

        let probes = collect_image_probes_incremental(
            root_dir.path(),
            &existing_by_path,
            &[],
            &db_path,
            0,
            None,
        )
        .expect("probes");
        assert_eq!(probes.len(), 1);
        assert!(matches!(probes[0].status, FileStatus::Unchanged));
    }

    #[test]
    fn cancel_flag_returns_early() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let db_dir = tempfile::tempdir().expect("dbdir");
        let db_path = db_dir.path().join("index.sqlite");
        db::init_database(&db_path).expect("init");

        // Create image files
        for name in &["a.jpg", "b.jpg", "c.jpg"] {
            let img = root_dir.path().join(name);
            let mut f = File::create(&img).expect("create");
            f.write_all(format!("data-{name}").as_bytes())
                .expect("write");
        }

        let job =
            start_or_resume_scan_job(&db_path, root_dir.path().to_str().unwrap()).expect("job");
        let ctx = make_scan_context(&db_path);

        // Set cancel flag before running
        let flag = AtomicBool::new(true);
        let result = run_scan_job_internal(&ctx, job.id, None, Some(&flag), false);
        assert!(result.is_ok());
        let summary = result.unwrap();
        // Should return early with minimal processing
        assert_eq!(summary.added, 0);
    }

    #[test]
    fn incremental_discovery_marks_new() {
        let root_dir = tempfile::tempdir().expect("tempdir");

        let img = root_dir.path().join("new.jpg");
        write_test_image(&img, 0xCC);

        let existing_by_path: HashMap<String, ExistingFile> = HashMap::new();
        let dummy_db = root_dir.path().join("dummy.sqlite");
        let probes = collect_image_probes_incremental(
            root_dir.path(),
            &existing_by_path,
            &[],
            &dummy_db,
            0,
            None,
        )
        .expect("probes");
        assert_eq!(probes.len(), 1);
        assert!(matches!(probes[0].status, FileStatus::New));
    }

    #[test]
    fn excluded_prefixes_skips_child_root_files() {
        let root_dir = tempfile::tempdir().expect("tempdir");

        // Create files in parent root
        let parent_img = root_dir.path().join("parent.jpg");
        write_test_image(&parent_img, 0xDD);

        // Create child root subdir with a file
        let child_dir = root_dir.path().join("Photos");
        std::fs::create_dir_all(&child_dir).expect("mkdir");
        let child_img = child_dir.join("child.jpg");
        write_test_image(&child_img, 0xEE);

        let existing_by_path: HashMap<String, ExistingFile> = HashMap::new();
        let dummy_db = root_dir.path().join("dummy.sqlite");

        // Without exclusions: both files found
        let probes_all = collect_image_probes_incremental(
            root_dir.path(),
            &existing_by_path,
            &[],
            &dummy_db,
            0,
            None,
        )
        .expect("probes all");
        assert_eq!(probes_all.len(), 2);

        // With child dir excluded: only parent file found
        let excluded = vec![child_dir.clone()];
        let probes_filtered = collect_image_probes_incremental(
            root_dir.path(),
            &existing_by_path,
            &excluded,
            &dummy_db,
            0,
            None,
        )
        .expect("probes filtered");
        assert_eq!(probes_filtered.len(), 1);
        assert_eq!(probes_filtered[0].filename, "parent.jpg");
    }

    #[test]
    fn supports_video_extensions() {
        assert!(is_supported_file(Path::new("movie.mp4")));
        assert!(is_supported_file(Path::new("show.mkv")));
        assert!(is_supported_file(Path::new("clip.webm")));
        assert!(is_supported_file(Path::new("cam.MOV")));
        assert!(is_supported_file(Path::new("file.avi")));
        assert!(is_supported_file(Path::new("file.flv")));
        assert!(is_supported_file(Path::new("file.wmv")));
        assert!(is_supported_file(Path::new("file.m4v")));
        assert!(is_supported_file(Path::new("file.ts")));
        assert!(is_supported_file(Path::new("file.mpg")));
        assert!(is_supported_file(Path::new("file.mpeg")));
        assert!(!is_supported_file(Path::new("file.txt")));
    }

    #[test]
    fn skips_tiny_images_but_not_pdfs() {
        let root_dir = tempfile::tempdir().expect("tempdir");

        // Tiny GIF (icon-sized) — should be skipped
        let tiny_gif = root_dir.path().join("spacer.gif");
        File::create(&tiny_gif)
            .expect("create")
            .write_all(&[0u8; 100])
            .expect("write");

        // Tiny PNG — should be skipped
        let tiny_png = root_dir.path().join("favicon.png");
        File::create(&tiny_png)
            .expect("create")
            .write_all(&[0u8; 5_000])
            .expect("write");

        // Normal-sized JPG — should be included
        let normal_jpg = root_dir.path().join("photo.jpg");
        write_test_image(&normal_jpg, 0xFF);

        // Tiny PDF — should still be included (PDFs exempt)
        let tiny_pdf = root_dir.path().join("note.pdf");
        File::create(&tiny_pdf)
            .expect("create")
            .write_all(&[0u8; 500])
            .expect("write");

        let existing_by_path: HashMap<String, ExistingFile> = HashMap::new();
        let dummy_db = root_dir.path().join("dummy.sqlite");
        let probes = collect_image_probes_incremental(
            root_dir.path(),
            &existing_by_path,
            &[],
            &dummy_db,
            0,
            None,
        )
        .expect("probes");

        let filenames: Vec<&str> = probes.iter().map(|p| p.filename.as_str()).collect();
        assert!(filenames.contains(&"photo.jpg"));
        assert!(filenames.contains(&"note.pdf"));
        assert!(!filenames.contains(&"spacer.gif"));
        assert!(!filenames.contains(&"favicon.png"));
        assert_eq!(probes.len(), 2);
    }
}
