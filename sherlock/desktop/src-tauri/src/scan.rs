use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::config::canonical_root_path;
use crate::db;
use crate::error::AppResult;
use crate::models::{ExistingFile, FileRecordUpsert, ScanSummary};

const IMAGE_EXTS: [&str; 8] = [
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".tiff", ".tif",
];

#[derive(Debug)]
struct FileProbe {
    rel_path: String,
    abs_path: String,
    filename: String,
    mtime_ns: i64,
    size_bytes: i64,
    fingerprint: String,
}

pub fn scan_root_and_sync(db_path: &Path, root_path: &str) -> AppResult<ScanSummary> {
    let started = Instant::now();
    let canonical_root = canonical_root_path(root_path)?;
    let root_string = canonical_root.display().to_string();
    let root_id = db::upsert_root(db_path, &root_string)?;
    let scan_marker = current_scan_marker();

    let existing = db::load_existing_files(db_path, root_id)?;
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

    let mut used_moved_ids = HashSet::new();
    let mut scanned = 0_u64;
    let mut added = 0_u64;
    let mut modified = 0_u64;
    let mut moved = 0_u64;
    let mut unchanged = 0_u64;

    for probe in collect_image_probes(&canonical_root)? {
        scanned += 1;
        if let Some(existing_file) = existing_by_path.get(&probe.rel_path) {
            let record = probe_to_record(root_id, scan_marker, &probe);
            if existing_file.fingerprint == probe.fingerprint {
                unchanged += 1;
            } else {
                modified += 1;
            }
            db::upsert_file_record(db_path, &record)?;
            continue;
        }

        if let Some(candidates) = by_fingerprint.get(&probe.fingerprint) {
            if let Some(candidate) = candidates
                .iter()
                .find(|c| !used_moved_ids.contains(&c.id) && c.rel_path != probe.rel_path)
            {
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
                    scan_marker,
                )?;
                continue;
            }
        }

        added += 1;
        let record = probe_to_record(root_id, scan_marker, &probe);
        db::upsert_file_record(db_path, &record)?;
    }

    let deleted = db::mark_missing_as_deleted(db_path, root_id, scan_marker)?;
    db::touch_root_scan(db_path, root_id)?;

    Ok(ScanSummary {
        root_id,
        root_path: root_string,
        scanned,
        added,
        modified,
        moved,
        unchanged,
        deleted,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn collect_image_probes(root: &Path) -> AppResult<Vec<FileProbe>> {
    let mut probes = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_image(path) {
            continue;
        }
        let probe = file_probe(root, path)?;
        probes.push(probe);
    }
    probes.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(probes)
}

fn file_probe(root: &Path, path: &Path) -> AppResult<FileProbe> {
    let metadata = std::fs::metadata(path)?;
    let rel = path
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let fingerprint = fingerprint_file(path, metadata.len())?;
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or_default();

    Ok(FileProbe {
        rel_path: rel,
        abs_path: path.display().to_string(),
        filename,
        mtime_ns,
        size_bytes: metadata.len() as i64,
        fingerprint,
    })
}

fn probe_to_record(root_id: i64, scan_marker: i64, probe: &FileProbe) -> FileRecordUpsert {
    FileRecordUpsert {
        root_id,
        rel_path: probe.rel_path.clone(),
        abs_path: probe.abs_path.clone(),
        filename: probe.filename.clone(),
        media_type: "other".to_string(),
        description: String::new(),
        extracted_text: String::new(),
        canonical_mentions: String::new(),
        confidence: 0.0,
        lang_hint: "unknown".to_string(),
        mtime_ns: probe.mtime_ns,
        size_bytes: probe.size_bytes,
        fingerprint: probe.fingerprint.clone(),
        scan_marker,
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

fn is_image(path: &Path) -> bool {
    let Some(ext) = path.extension().map(|v| v.to_string_lossy().to_lowercase()) else {
        return false;
    };
    let ext = format!(".{ext}");
    IMAGE_EXTS.contains(&ext.as_str())
}

fn current_scan_marker() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::db;

    #[test]
    fn detects_move_without_reinsert() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let db_dir = tempfile::tempdir().expect("dbdir");
        let db_path = db_dir.path().join("index.sqlite");
        db::init_database(&db_path).expect("init");

        let image_a = root_dir.path().join("a.jpg");
        let mut f = File::create(&image_a).expect("create");
        f.write_all(b"same-binary").expect("write");

        let first = scan_root_and_sync(&db_path, root_dir.path().to_str().expect("str"))
            .expect("first scan");
        assert_eq!(first.added, 1);

        let moved = root_dir.path().join("moved.jpg");
        std::fs::rename(&image_a, &moved).expect("rename");
        let second = scan_root_and_sync(&db_path, root_dir.path().to_str().expect("str"))
            .expect("second scan");
        assert_eq!(second.moved, 1);
        assert_eq!(second.added, 0);
    }
}
