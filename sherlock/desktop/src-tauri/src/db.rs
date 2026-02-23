use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rusqlite::{params, params_from_iter, types::Value, Connection, OpenFlags, Row};
use rusqlite_migration::{HookError, Migrations, M};

use crate::error::{AppError, AppResult};
use crate::models::{
    Album, DbStats, ExistingFile, FileMetadata, FileRecordUpsert, HealthCheckOutcome, ParsedQuery,
    PurgeResult, RootInfo, ScanJobState, ScanJobStatus, SearchItem, SearchRequest, SearchResponse,
    SmartFolder, SortField, SortOrder,
};
use crate::query_parser::parse_query;

const DEFAULT_LIMIT: u32 = 80;
const MAX_LIMIT: u32 = 200;

/// Centralized connection helper. Sets busy_timeout and foreign_keys on every
/// connection so CASCADE constraints are active and concurrent access doesn't
/// fail immediately with SQLITE_BUSY.
///
/// If the filesystem is read-only (e.g. sandbox, mounted RO), falls back to
/// opening the database in read-only mode so queries still work.
fn open_conn(db_path: &Path) -> AppResult<Connection> {
    match try_open_rw(db_path) {
        Ok(conn) => Ok(conn),
        Err(ref e) if is_readonly_error(e) => {
            let conn = Connection::open_with_flags(
                db_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            conn.pragma_update(None, "busy_timeout", 5000)?;
            Ok(conn)
        }
        Err(e) => Err(e),
    }
}

fn try_open_rw(db_path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

fn is_readonly_error(e: &AppError) -> bool {
    match e {
        AppError::Db(rusqlite::Error::SqliteFailure(f, _)) => {
            f.extended_code == 14 || f.code == rusqlite::ErrorCode::ReadOnly
        }
        _ => false,
    }
}

pub fn init_database(db_path: &Path) -> AppResult<()> {
    let mut conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    run_migrations(&mut conn)?;
    Ok(())
}

fn run_migrations(conn: &mut Connection) -> AppResult<()> {
    let migrations = Migrations::new(vec![
        // Migration 0: Full initial schema.
        // Uses IF NOT EXISTS so it works on both fresh and pre-migration databases
        // (where user_version is 0 but tables already exist).
        M::up_with_hook(
            r#"
            CREATE TABLE IF NOT EXISTS roots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                root_path TEXT NOT NULL UNIQUE,
                root_name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_scan_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                root_id INTEGER NOT NULL,
                rel_path TEXT NOT NULL,
                filename TEXT NOT NULL,
                abs_path TEXT NOT NULL,
                media_type TEXT NOT NULL DEFAULT 'other',
                description TEXT NOT NULL DEFAULT '',
                extracted_text TEXT NOT NULL DEFAULT '',
                canonical_mentions TEXT NOT NULL DEFAULT '',
                confidence REAL NOT NULL DEFAULT 0.0,
                lang_hint TEXT NOT NULL DEFAULT 'unknown',
                mtime_ns INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                fingerprint TEXT NOT NULL,
                thumb_path TEXT,
                scan_marker INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER,
                UNIQUE(root_id, rel_path),
                FOREIGN KEY (root_id) REFERENCES roots(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_files_root ON files(root_id);
            CREATE INDEX IF NOT EXISTS idx_files_media_type ON files(media_type);
            CREATE INDEX IF NOT EXISTS idx_files_confidence ON files(confidence);
            CREATE INDEX IF NOT EXISTS idx_files_updated_at ON files(updated_at);
            CREATE INDEX IF NOT EXISTS idx_files_fingerprint ON files(fingerprint);
            CREATE INDEX IF NOT EXISTS idx_files_deleted_at ON files(deleted_at);
            CREATE INDEX IF NOT EXISTS idx_files_mtime_ns ON files(mtime_ns);
            CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename);

            CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                filename,
                rel_path,
                description,
                extracted_text,
                canonical_mentions
            );

            CREATE TABLE IF NOT EXISTS scan_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                root_id INTEGER NOT NULL,
                root_path TEXT NOT NULL,
                status TEXT NOT NULL,
                scan_marker INTEGER NOT NULL,
                total_files INTEGER NOT NULL DEFAULT 0,
                processed_files INTEGER NOT NULL DEFAULT 0,
                added INTEGER NOT NULL DEFAULT 0,
                modified INTEGER NOT NULL DEFAULT 0,
                moved INTEGER NOT NULL DEFAULT 0,
                unchanged INTEGER NOT NULL DEFAULT 0,
                deleted INTEGER NOT NULL DEFAULT 0,
                cursor_rel_path TEXT,
                error_text TEXT,
                started_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                completed_at INTEGER,
                FOREIGN KEY (root_id) REFERENCES roots(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_scan_jobs_root ON scan_jobs(root_id);
            CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status);
            CREATE INDEX IF NOT EXISTS idx_scan_jobs_updated_at ON scan_jobs(updated_at);
            "#,
            |conn| ensure_fts_schema(conn).map_err(|e| HookError::Hook(e.to_string())),
        ),
        // -------------------------------------------------------------------
        // HOW TO ADD A NEW MIGRATION:
        // 1. Append a new M::up("...") entry here. Never edit or reorder
        //    existing migrations — they are identified by position.
        // 2. Each migration runs inside a transaction. Keep statements small
        //    and idempotent where possible.
        // 3. Example:
        //    M::up("ALTER TABLE files ADD COLUMN tags TEXT NOT NULL DEFAULT '';"),
        // -------------------------------------------------------------------
        // Migration 1: Add location_text column
        M::up("ALTER TABLE files ADD COLUMN location_text TEXT NOT NULL DEFAULT '';"),
        // Migration 2: Rebuild FTS5 with location_text column
        M::up_with_hook("SELECT 1;", |conn| {
            rebuild_fts_with_location(conn).map_err(|e| HookError::Hook(e.to_string()))
        }),
        // Migration 3: Albums + album_files
        M::up(
            r#"
            CREATE TABLE IF NOT EXISTS albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS album_files (
                album_id INTEGER NOT NULL,
                file_id INTEGER NOT NULL,
                added_at INTEGER NOT NULL,
                PRIMARY KEY (album_id, file_id),
                FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE,
                FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_album_files_file ON album_files(file_id);
            "#,
        ),
        // Migration 4: Smart folders
        M::up(
            r#"
            CREATE TABLE IF NOT EXISTS smart_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                query TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            "#,
        ),
    ]);

    migrations
        .to_latest(conn)
        .map_err(|e| AppError::Config(format!("migration error: {e}")))?;
    Ok(())
}

pub fn recover_incomplete_scan_jobs(db_path: &Path) -> AppResult<u64> {
    let conn = open_conn(db_path)?;
    let now = now_epoch_secs();
    let updated = conn.execute(
        "UPDATE scan_jobs
         SET status = 'interrupted', updated_at = ?1
         WHERE status = 'running'",
        params![now],
    )?;
    Ok(updated as u64)
}

pub fn database_stats(db_path: &Path) -> AppResult<DbStats> {
    let conn = open_conn(db_path)?;
    let roots: i64 = conn.query_row("SELECT COUNT(*) FROM roots", [], |r| r.get(0))?;
    let files: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(DbStats {
        roots: roots as u64,
        files: files as u64,
        db_size_bytes: 0,
        thumbs_size_bytes: 0,
    })
}

#[cfg(test)]
pub fn upsert_root(db_path: &Path, root_path: &str) -> AppResult<i64> {
    let conn = open_conn(db_path)?;
    upsert_root_conn(&conn, root_path)
}

fn upsert_root_conn(conn: &Connection, root_path: &str) -> AppResult<i64> {
    if let Ok(id) = conn.query_row(
        "SELECT id FROM roots WHERE root_path = ?1",
        params![root_path],
        |r| r.get(0),
    ) {
        return Ok(id);
    }

    let root_name = std::path::Path::new(root_path)
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "root".to_string());
    let now = now_epoch_secs();
    conn.execute(
        "INSERT INTO roots(root_path, root_name, created_at, last_scan_at) VALUES (?1, ?2, ?3, ?4)",
        params![root_path, root_name, now, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn touch_root_scan(db_path: &Path, root_id: i64) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "UPDATE roots SET last_scan_at = ?2 WHERE id = ?1",
        params![root_id, now_epoch_secs()],
    )?;
    Ok(())
}

pub fn create_or_resume_scan_job(db_path: &Path, root_path: &str) -> AppResult<ScanJobStatus> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    let root_id = upsert_root_conn(&tx, root_path)?;
    let now = now_epoch_secs();

    let maybe_job_id: Option<i64> = tx
        .query_row(
            "SELECT id
             FROM scan_jobs
             WHERE root_id = ?1 AND status IN ('running', 'pending', 'interrupted', 'failed')
             ORDER BY id DESC
             LIMIT 1",
            params![root_id],
            |row| row.get(0),
        )
        .ok();

    if let Some(job_id) = maybe_job_id {
        tx.execute(
            "UPDATE scan_jobs
             SET status = 'running', error_text = NULL,
                 cursor_rel_path = NULL, processed_files = 0,
                 added = 0, modified = 0, moved = 0, unchanged = 0,
                 updated_at = ?2
             WHERE id = ?1",
            params![job_id, now],
        )?;
        tx.commit()?;
        return get_scan_job(db_path, job_id)?
            .ok_or_else(|| AppError::Config("missing scan job after resume".to_string()));
    }

    let scan_marker = now_epoch_millis();
    tx.execute(
        "INSERT INTO scan_jobs(
            root_id, root_path, status, scan_marker,
            total_files, processed_files, added, modified, moved, unchanged, deleted,
            cursor_rel_path, error_text, started_at, updated_at, completed_at
         ) VALUES (
            ?1, ?2, 'running', ?3,
            0, 0, 0, 0, 0, 0, 0,
            NULL, NULL, ?4, ?4, NULL
         )",
        params![root_id, root_path, scan_marker, now],
    )?;
    let job_id = tx.last_insert_rowid();
    tx.commit()?;
    get_scan_job(db_path, job_id)?
        .ok_or_else(|| AppError::Config("missing scan job after insert".to_string()))
}

pub fn list_resumable_scan_jobs(db_path: &Path) -> AppResult<Vec<ScanJobStatus>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT
            id, root_id, root_path, status, scan_marker, total_files, processed_files,
            added, modified, moved, unchanged, deleted, cursor_rel_path, error_text,
            updated_at, started_at, completed_at
         FROM scan_jobs
         WHERE status IN ('running', 'pending', 'interrupted')
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], scan_job_from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_scan_job(db_path: &Path, job_id: i64) -> AppResult<Option<ScanJobStatus>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT
            id, root_id, root_path, status, scan_marker, total_files, processed_files,
            added, modified, moved, unchanged, deleted, cursor_rel_path, error_text,
            updated_at, started_at, completed_at
         FROM scan_jobs
         WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![job_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(scan_job_from_row(row)?));
    }
    Ok(None)
}

pub fn get_scan_job_state(db_path: &Path, job_id: i64) -> AppResult<ScanJobState> {
    let conn = open_conn(db_path)?;
    conn.query_row(
        "SELECT
            root_id, root_path, scan_marker, processed_files,
            added, modified, moved, unchanged, cursor_rel_path
         FROM scan_jobs
         WHERE id = ?1",
        params![job_id],
        |row| {
            Ok(ScanJobState {
                root_id: row.get(0)?,
                root_path: row.get(1)?,
                scan_marker: row.get(2)?,
                processed_files: row.get::<_, i64>(3)? as u64,
                added: row.get::<_, i64>(4)? as u64,
                modified: row.get::<_, i64>(5)? as u64,
                moved: row.get::<_, i64>(6)? as u64,
                unchanged: row.get::<_, i64>(7)? as u64,
                cursor_rel_path: row.get(8)?,
            })
        },
    )
    .map_err(AppError::from)
}

#[allow(clippy::too_many_arguments)]
pub fn checkpoint_scan_job(
    db_path: &Path,
    job_id: i64,
    total_files: u64,
    processed_files: u64,
    cursor_rel_path: Option<&str>,
    added: u64,
    modified: u64,
    moved: u64,
    unchanged: u64,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "UPDATE scan_jobs
         SET status = 'running',
             total_files = ?2,
             processed_files = ?3,
             cursor_rel_path = ?4,
             added = ?5,
             modified = ?6,
             moved = ?7,
             unchanged = ?8,
             updated_at = ?9
         WHERE id = ?1",
        params![
            job_id,
            total_files as i64,
            processed_files as i64,
            cursor_rel_path,
            added as i64,
            modified as i64,
            moved as i64,
            unchanged as i64,
            now_epoch_secs()
        ],
    )?;
    Ok(())
}

pub fn complete_scan_job_by_id(
    db_path: &Path,
    job_id: i64,
    summary: &crate::models::ScanSummary,
    cursor_rel_path: Option<&str>,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    let now = now_epoch_secs();
    conn.execute(
        "UPDATE scan_jobs
         SET status = 'completed',
             total_files = ?2,
             processed_files = ?3,
             added = ?4,
             modified = ?5,
             moved = ?6,
             unchanged = ?7,
             deleted = ?8,
             cursor_rel_path = ?9,
             updated_at = ?10,
             completed_at = ?10,
             error_text = NULL
         WHERE id = ?1",
        params![
            job_id,
            summary.scanned as i64,
            summary.scanned as i64,
            summary.added as i64,
            summary.modified as i64,
            summary.moved as i64,
            summary.unchanged as i64,
            summary.deleted as i64,
            cursor_rel_path,
            now
        ],
    )?;
    Ok(())
}

pub fn cancel_scan_job(db_path: &Path, job_id: i64) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "UPDATE scan_jobs SET status = 'interrupted', error_text = 'cancelled by user', updated_at = ?2
         WHERE id = ?1 AND status = 'running'",
        params![job_id, now_epoch_secs()],
    )?;
    Ok(())
}

pub fn fail_scan_job(db_path: &Path, job_id: i64, error_text: &str) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "UPDATE scan_jobs
         SET status = 'failed',
             error_text = ?2,
             updated_at = ?3
         WHERE id = ?1",
        params![job_id, truncate_text(error_text, 1500), now_epoch_secs()],
    )?;
    Ok(())
}

pub fn load_existing_files(db_path: &Path, root_id: i64) -> AppResult<Vec<ExistingFile>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, fingerprint, mtime_ns, size_bytes, confidence
         FROM files
         WHERE root_id = ?1 AND deleted_at IS NULL",
    )?;

    let rows = stmt.query_map(params![root_id], |row| {
        Ok(ExistingFile {
            id: row.get(0)?,
            rel_path: row.get(1)?,
            fingerprint: row.get(2)?,
            mtime_ns: row.get(3)?,
            size_bytes: row.get(4)?,
            confidence: row.get::<_, f64>(5)? as f32,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn touch_file_scan_marker(
    db_path: &Path,
    root_id: i64,
    rel_path: &str,
    scan_marker: i64,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "UPDATE files SET scan_marker = ?1, updated_at = ?2 WHERE root_id = ?3 AND rel_path = ?4",
        params![scan_marker, now_epoch_secs(), root_id, rel_path],
    )?;
    Ok(())
}

pub fn get_deleted_file_paths(
    db_path: &Path,
    root_id: i64,
    deleted_at: i64,
) -> AppResult<Vec<(String, Option<String>)>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn
        .prepare("SELECT rel_path, thumb_path FROM files WHERE root_id = ?1 AND deleted_at = ?2")?;
    let rows = stmt.query_map(params![root_id, deleted_at], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update_file_thumb_path(
    db_path: &Path,
    root_id: i64,
    rel_path: &str,
    thumb_path: &str,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "UPDATE files SET thumb_path = ?1, updated_at = ?2 WHERE root_id = ?3 AND rel_path = ?4",
        params![thumb_path, now_epoch_secs(), root_id, rel_path],
    )?;
    Ok(())
}

pub fn upsert_file_record(db_path: &Path, record: &FileRecordUpsert) -> AppResult<i64> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    let now = now_epoch_secs();
    tx.execute(
        r#"
        INSERT INTO files (
            root_id, rel_path, filename, abs_path,
            media_type, description, extracted_text, canonical_mentions,
            confidence, lang_hint, mtime_ns, size_bytes, fingerprint,
            scan_marker, updated_at, deleted_at, location_text
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, NULL, ?16
        )
        ON CONFLICT(root_id, rel_path) DO UPDATE SET
            filename = excluded.filename,
            abs_path = excluded.abs_path,
            media_type = excluded.media_type,
            description = excluded.description,
            extracted_text = excluded.extracted_text,
            canonical_mentions = excluded.canonical_mentions,
            confidence = excluded.confidence,
            lang_hint = excluded.lang_hint,
            mtime_ns = excluded.mtime_ns,
            size_bytes = excluded.size_bytes,
            fingerprint = excluded.fingerprint,
            scan_marker = excluded.scan_marker,
            updated_at = excluded.updated_at,
            deleted_at = NULL,
            location_text = excluded.location_text
        "#,
        params![
            record.root_id,
            record.rel_path,
            record.filename,
            record.abs_path,
            record.media_type,
            record.description,
            record.extracted_text,
            record.canonical_mentions,
            record.confidence,
            record.lang_hint,
            record.mtime_ns,
            record.size_bytes,
            record.fingerprint,
            record.scan_marker,
            now,
            record.location_text
        ],
    )?;

    let file_id: i64 = tx.query_row(
        "SELECT id FROM files WHERE root_id = ?1 AND rel_path = ?2",
        params![record.root_id, record.rel_path],
        |r| r.get(0),
    )?;
    refresh_fts(&tx, file_id)?;
    tx.commit()?;
    Ok(file_id)
}

#[allow(clippy::too_many_arguments)]
pub fn move_file_by_id(
    db_path: &Path,
    file_id: i64,
    rel_path: &str,
    abs_path: &str,
    filename: &str,
    mtime_ns: i64,
    size_bytes: i64,
    scan_marker: i64,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        r#"
        UPDATE files
        SET rel_path = ?2,
            abs_path = ?3,
            filename = ?4,
            mtime_ns = ?5,
            size_bytes = ?6,
            scan_marker = ?7,
            updated_at = ?8,
            deleted_at = NULL
        WHERE id = ?1
        "#,
        params![
            file_id,
            rel_path,
            abs_path,
            filename,
            mtime_ns,
            size_bytes,
            scan_marker,
            now_epoch_secs()
        ],
    )?;
    refresh_fts(&tx, file_id)?;
    tx.commit()?;
    Ok(())
}

pub fn mark_missing_as_deleted(db_path: &Path, root_id: i64, scan_marker: i64) -> AppResult<u64> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    let now = now_epoch_secs();

    // Collect IDs of files about to be soft-deleted
    let mut stmt = tx.prepare(
        "SELECT id FROM files WHERE root_id = ?1 AND deleted_at IS NULL AND scan_marker <> ?2",
    )?;
    let ids: Vec<i64> = stmt
        .query_map(params![root_id, scan_marker], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    if ids.is_empty() {
        tx.commit()?;
        return Ok(0);
    }

    // Soft-delete files
    tx.execute(
        "UPDATE files
         SET deleted_at = ?3, updated_at = ?3
         WHERE root_id = ?1
           AND deleted_at IS NULL
           AND scan_marker <> ?2",
        params![root_id, scan_marker, now],
    )?;

    // Remove their FTS entries
    for id in &ids {
        tx.execute("DELETE FROM files_fts WHERE rowid = ?1", params![id])?;
    }

    tx.commit()?;
    Ok(ids.len() as u64)
}

fn refresh_fts(conn: &Connection, file_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM files_fts WHERE rowid = ?1", params![file_id])?;
    conn.execute(
        r#"
        INSERT INTO files_fts (rowid, filename, rel_path, description, extracted_text,
                               canonical_mentions, location_text)
        SELECT id, filename, rel_path, description, extracted_text,
               canonical_mentions, location_text
        FROM files
        WHERE id = ?1
        "#,
        params![file_id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Root management
// ---------------------------------------------------------------------------

pub fn list_roots(db_path: &Path) -> AppResult<Vec<RootInfo>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT r.id, r.root_path, r.root_name, r.created_at, r.last_scan_at,
                (SELECT COUNT(*) FROM files f WHERE f.root_id = r.id AND f.deleted_at IS NULL)
         FROM roots r ORDER BY r.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RootInfo {
            id: row.get(0)?,
            root_path: row.get(1)?,
            root_name: row.get(2)?,
            created_at: row.get(3)?,
            last_scan_at: row.get(4)?,
            file_count: row.get::<_, i64>(5)? as u64,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn purge_root(db_path: &Path, root_id: i64) -> AppResult<PurgeResult> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;

    // Collect file IDs and thumb_paths for cleanup
    let mut stmt = tx.prepare("SELECT id, thumb_path FROM files WHERE root_id = ?1")?;
    let file_rows: Vec<(i64, Option<String>)> = stmt
        .query_map(params![root_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let file_ids: Vec<i64> = file_rows.iter().map(|(id, _)| *id).collect();
    let thumb_paths: Vec<String> = file_rows.iter().filter_map(|(_, tp)| tp.clone()).collect();

    // Delete FTS entries
    for id in &file_ids {
        tx.execute("DELETE FROM files_fts WHERE rowid = ?1", params![id])?;
    }

    // Delete files
    let files_removed = tx.execute("DELETE FROM files WHERE root_id = ?1", params![root_id])?;

    // Delete scan jobs
    let jobs_removed = tx.execute("DELETE FROM scan_jobs WHERE root_id = ?1", params![root_id])?;

    // Delete root
    tx.execute("DELETE FROM roots WHERE id = ?1", params![root_id])?;

    tx.commit()?;

    // Best-effort thumbnail cleanup (outside transaction)
    let mut thumbs_cleaned = 0u64;
    for tp in &thumb_paths {
        let path = Path::new(tp);
        if path.exists() && std::fs::remove_file(path).is_ok() {
            thumbs_cleaned += 1;
        }
    }

    Ok(PurgeResult {
        files_removed: files_removed as u64,
        jobs_removed: jobs_removed as u64,
        thumbs_cleaned,
    })
}

// ---------------------------------------------------------------------------
// Health check & backup
// ---------------------------------------------------------------------------

pub fn quick_check(db_path: &Path) -> AppResult<bool> {
    let conn = open_conn(db_path)?;
    let result: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    Ok(result == "ok")
}

pub fn wal_checkpoint(db_path: &Path) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    Ok(())
}

pub fn backup_database(db_path: &Path, backup_path: &Path) -> AppResult<()> {
    use rusqlite::backup::Backup;
    let src = open_conn(db_path)?;
    let mut dst = Connection::open(backup_path)?;
    let backup = Backup::new(&src, &mut dst)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(50), None)?;
    Ok(())
}

pub fn restore_from_backup(backup_path: &Path, db_path: &Path) -> AppResult<()> {
    if !backup_path.exists() {
        return Err(AppError::Config("backup file does not exist".to_string()));
    }
    // Remove main DB + WAL + SHM
    let _ = std::fs::remove_file(db_path);
    let wal = db_path.with_extension("sqlite-wal");
    let shm = db_path.with_extension("sqlite-shm");
    let _ = std::fs::remove_file(&wal);
    let _ = std::fs::remove_file(&shm);

    std::fs::copy(backup_path, db_path)?;
    init_database(db_path)?;
    Ok(())
}

pub fn recreate_database(db_path: &Path) -> AppResult<()> {
    let _ = std::fs::remove_file(db_path);
    let wal = db_path.with_extension("sqlite-wal");
    let shm = db_path.with_extension("sqlite-shm");
    let _ = std::fs::remove_file(&wal);
    let _ = std::fs::remove_file(&shm);
    init_database(db_path)
}

pub fn startup_health_check(db_path: &Path, backup_dir: &Path) -> AppResult<HealthCheckOutcome> {
    if !db_path.exists() {
        return Ok(HealthCheckOutcome::Healthy);
    }
    if let Ok(true) = quick_check(db_path) {
        return Ok(HealthCheckOutcome::Healthy);
    }
    // Database is corrupt — attempt restore
    let backup_path = backup_dir.join("index.sqlite.bak");
    if backup_path.exists() && restore_from_backup(&backup_path, db_path).is_ok() {
        return Ok(HealthCheckOutcome::RestoredFromBackup);
    }
    // No backup or restore failed — recreate
    recreate_database(db_path)?;
    Ok(HealthCheckOutcome::Recreated)
}

pub fn validate_and_purge_stale_roots(
    db_path: &Path,
    thumbnails_dir: &Path,
) -> AppResult<Vec<String>> {
    let roots = list_roots(db_path)?;
    let mut purged = Vec::new();
    for root in roots {
        if !Path::new(&root.root_path).is_dir() {
            let result = purge_root(db_path, root.id)?;
            // Also try to clean up the thumbnail subtree for this root
            let thumb_root = thumbnails_dir.join(&root.root_name);
            if thumb_root.is_dir() {
                let _ = std::fs::remove_dir_all(&thumb_root);
            }
            log::info!(
                "Purged stale root '{}': {} files, {} jobs, {} thumbs cleaned",
                root.root_path,
                result.files_removed,
                result.jobs_removed,
                result.thumbs_cleaned
            );
            purged.push(root.root_path);
        }
    }
    Ok(purged)
}

// ---------------------------------------------------------------------------
// File delete / rename
// ---------------------------------------------------------------------------

pub fn get_file_path_info(
    db_path: &Path,
    file_id: i64,
) -> AppResult<(String, String, Option<String>)> {
    let conn = open_conn(db_path)?;
    conn.query_row(
        "SELECT abs_path, rel_path, thumb_path FROM files WHERE id = ?1 AND deleted_at IS NULL",
        params![file_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .map_err(AppError::from)
}

/// Collect file info needed for deletion without mutating the DB.
/// Returns `(file_id, abs_path, thumb_path)` for each existing, non-deleted file.
pub fn collect_files_for_delete(
    db_path: &Path,
    file_ids: &[i64],
) -> AppResult<Vec<(i64, String, Option<String>)>> {
    let conn = open_conn(db_path)?;
    let mut results = Vec::new();
    for &fid in file_ids {
        let info: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT abs_path, thumb_path FROM files WHERE id = ?1 AND deleted_at IS NULL",
                params![fid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        if let Some((abs_path, thumb_path)) = info {
            results.push((fid, abs_path, thumb_path));
        }
    }
    Ok(results)
}

/// Delete file records from DB for the given ids (FTS + files table).
/// Only call this for files that have already been removed from the filesystem.
pub fn delete_file_records(db_path: &Path, file_ids: &[i64]) -> AppResult<()> {
    if file_ids.is_empty() {
        return Ok(());
    }
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    for &fid in file_ids {
        tx.execute("DELETE FROM files_fts WHERE rowid = ?1", params![fid])?;
        tx.execute("DELETE FROM files WHERE id = ?1", params![fid])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn rename_file_record(
    db_path: &Path,
    file_id: i64,
    new_rel_path: &str,
    new_abs_path: &str,
    new_filename: &str,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    let now = now_epoch_secs();
    tx.execute(
        "UPDATE files SET rel_path = ?2, abs_path = ?3, filename = ?4, updated_at = ?5 WHERE id = ?1",
        params![file_id, new_rel_path, new_abs_path, new_filename, now],
    )?;
    refresh_fts(&tx, file_id)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// File metadata get/update
// ---------------------------------------------------------------------------

pub fn get_file_metadata(db_path: &Path, file_id: i64) -> AppResult<FileMetadata> {
    let conn = open_conn(db_path)?;
    conn.query_row(
        "SELECT id, media_type, description, extracted_text, canonical_mentions, location_text
         FROM files WHERE id = ?1 AND deleted_at IS NULL",
        params![file_id],
        |row| {
            Ok(FileMetadata {
                id: row.get(0)?,
                media_type: row.get(1)?,
                description: row.get(2)?,
                extracted_text: row.get(3)?,
                canonical_mentions: row.get(4)?,
                location_text: row.get(5)?,
            })
        },
    )
    .map_err(AppError::from)
}

#[allow(clippy::too_many_arguments)]
pub fn update_file_metadata(
    db_path: &Path,
    file_id: i64,
    media_type: &str,
    description: &str,
    extracted_text: &str,
    canonical_mentions: &str,
    location_text: &str,
) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    let tx = conn.unchecked_transaction()?;
    let now = now_epoch_secs();
    tx.execute(
        "UPDATE files SET media_type = ?2, description = ?3, extracted_text = ?4,
                canonical_mentions = ?5, location_text = ?6, updated_at = ?7
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            file_id,
            media_type,
            description,
            extracted_text,
            canonical_mentions,
            location_text,
            now
        ],
    )?;
    refresh_fts(&tx, file_id)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

pub fn search_images(db_path: &Path, request: &SearchRequest) -> AppResult<SearchResponse> {
    let parsed = parse_query(&request.query);
    let normalized = normalize_request(request, &parsed);
    let inferred_media = request.media_types.is_empty() && !parsed.media_types.is_empty();
    let inferred_conf = request.min_confidence.is_none() && parsed.min_confidence.is_some();
    let inferred_date_from = request.date_from.is_none() && parsed.date_from.is_some();
    let inferred_date_to = request.date_to.is_none() && parsed.date_to.is_some();

    let initial = search_images_normalized(db_path, normalized.clone(), parsed.clone())?;
    if initial.total > 0
        || !(inferred_media || inferred_conf || inferred_date_from || inferred_date_to)
    {
        return Ok(initial);
    }

    // If parser-inferred filters are over-restrictive, retry with relaxed constraints.
    let mut relaxed = normalized;
    if inferred_media {
        relaxed.media_types.clear();
    }
    if inferred_conf {
        relaxed.min_confidence = None;
    }
    if inferred_date_from {
        relaxed.date_from = None;
    }
    if inferred_date_to {
        relaxed.date_to = None;
    }
    search_images_normalized(db_path, relaxed, parsed)
}

fn normalize_request(request: &SearchRequest, parsed: &ParsedQuery) -> SearchRequest {
    let mut normalized = request.clone();
    normalized.limit = Some(request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT));
    normalized.offset = Some(request.offset.unwrap_or(0));

    if normalized.media_types.is_empty() && !parsed.media_types.is_empty() {
        normalized.media_types = parsed.media_types.clone();
    }
    if normalized.min_confidence.is_none() {
        normalized.min_confidence = parsed.min_confidence;
    }
    if normalized.date_from.is_none() {
        normalized.date_from = parsed.date_from.clone();
    }
    if normalized.date_to.is_none() {
        normalized.date_to = parsed.date_to.clone();
    }
    normalized
}

fn search_images_normalized(
    db_path: &Path,
    request: SearchRequest,
    parsed: ParsedQuery,
) -> AppResult<SearchResponse> {
    let conn = open_conn(db_path)?;
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = request.offset.unwrap_or(0);
    // Use parsed query_text (which strips album: prefix) for FTS matching
    let query = parsed.query_text.trim().to_string();
    let fts_query = to_fts_query(&query);
    let has_query = !fts_query.is_empty();

    let mut from_sql = String::from(" FROM files f ");
    let mut where_clauses = vec!["f.deleted_at IS NULL".to_string()];
    let mut bind_values: Vec<Value> = Vec::new();

    if has_query {
        from_sql.push_str(" JOIN files_fts ON files_fts.rowid = f.id ");
        where_clauses.push("files_fts MATCH ?".to_string());
        bind_values.push(Value::Text(fts_query));
    }

    if !request.root_scope.is_empty() {
        let placeholders = vec!["?"; request.root_scope.len()].join(", ");
        where_clauses.push(format!("f.root_id IN ({placeholders})"));
        for root_id in &request.root_scope {
            bind_values.push(Value::Integer(*root_id));
        }
    }

    if let Some(album_name) = &parsed.album_name {
        from_sql.push_str(
            " JOIN album_files af ON af.file_id = f.id \
             JOIN albums a ON a.id = af.album_id ",
        );
        where_clauses.push("a.name = ? COLLATE NOCASE".to_string());
        bind_values.push(Value::Text(album_name.clone()));
    }

    let media_types = normalize_media_types(&request.media_types);
    if !media_types.is_empty() {
        let placeholders = vec!["?"; media_types.len()].join(", ");
        where_clauses.push(format!("f.media_type IN ({placeholders})"));
        for media in media_types {
            bind_values.push(Value::Text(media));
        }
    }

    if let Some(min_conf) = request.min_confidence {
        where_clauses.push("f.confidence >= ?".to_string());
        bind_values.push(Value::Real(min_conf.clamp(0.0, 1.0) as f64));
    }

    if let Some(start_ns) = request
        .date_from
        .as_ref()
        .and_then(|v| parse_date_start_ns(v))
    {
        where_clauses.push("f.mtime_ns >= ?".to_string());
        bind_values.push(Value::Integer(start_ns));
    }
    if let Some(end_ns) = request.date_to.as_ref().and_then(|v| parse_date_end_ns(v)) {
        where_clauses.push("f.mtime_ns <= ?".to_string());
        bind_values.push(Value::Integer(end_ns));
    }

    let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));
    let count_sql = format!("SELECT COUNT(*){}{}", from_sql, where_sql);
    let mut count_stmt = conn.prepare(&count_sql)?;
    let total: i64 = count_stmt.query_row(params_from_iter(bind_values.clone()), |r| r.get(0))?;

    let order_sql = build_order_clause(has_query, &request.sort_by, &request.sort_order);
    let select_sql = format!(
        "SELECT f.id, f.root_id, f.rel_path, f.abs_path, f.media_type, f.description, \
         f.confidence, f.mtime_ns, f.size_bytes, f.thumb_path{}{}{} LIMIT ? OFFSET ?",
        from_sql, where_sql, order_sql
    );
    let mut select_bind = bind_values;
    select_bind.push(Value::Integer(limit as i64));
    select_bind.push(Value::Integer(offset as i64));
    let mut stmt = conn.prepare(&select_sql)?;
    let rows = stmt.query_map(params_from_iter(select_bind), |row| {
        Ok(SearchItem {
            id: row.get(0)?,
            root_id: row.get(1)?,
            rel_path: row.get(2)?,
            abs_path: row.get(3)?,
            media_type: row.get(4)?,
            description: row.get(5)?,
            confidence: row.get::<_, f64>(6)? as f32,
            mtime_ns: row.get(7)?,
            size_bytes: row.get(8)?,
            thumbnail_path: row.get(9)?,
        })
    })?;

    let mut items = Vec::new();
    for item in rows {
        let item = item?;
        if items.len() < 3 {
            log::info!(
                "[search_debug] id={} rel_path={} thumb_path={:?}",
                item.id,
                item.rel_path,
                item.thumbnail_path
            );
        }
        items.push(item);
    }

    Ok(SearchResponse {
        total: total as u64,
        limit,
        offset,
        items,
        parsed_query: parsed,
    })
}

fn build_order_clause(has_query: bool, sort_by: &SortField, sort_order: &SortOrder) -> String {
    let dir = match sort_order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };
    let primary = match sort_by {
        SortField::Relevance => {
            if has_query {
                return " ORDER BY bm25(files_fts), f.confidence DESC, f.updated_at DESC, f.id DESC ".to_string();
            }
            // No query text — fall back to date modified
            format!("f.mtime_ns {dir}")
        }
        SortField::DateModified => format!("f.mtime_ns {dir}"),
        SortField::Name => format!("f.filename {dir}"),
        SortField::Type => format!("f.media_type {dir}"),
    };
    format!(" ORDER BY {primary}, f.updated_at DESC, f.id DESC ")
}

fn scan_job_from_row(row: &Row<'_>) -> rusqlite::Result<ScanJobStatus> {
    let total_files = row.get::<_, i64>(5)? as u64;
    let processed_files = row.get::<_, i64>(6)? as u64;
    let progress_pct = if total_files == 0 {
        0.0
    } else {
        ((processed_files as f32 / total_files as f32) * 100.0).clamp(0.0, 100.0)
    };
    Ok(ScanJobStatus {
        id: row.get(0)?,
        root_id: row.get(1)?,
        root_path: row.get(2)?,
        status: row.get(3)?,
        scan_marker: row.get(4)?,
        total_files,
        processed_files,
        progress_pct,
        added: row.get::<_, i64>(7)? as u64,
        modified: row.get::<_, i64>(8)? as u64,
        moved: row.get::<_, i64>(9)? as u64,
        unchanged: row.get::<_, i64>(10)? as u64,
        deleted: row.get::<_, i64>(11)? as u64,
        cursor_rel_path: row.get(12)?,
        error_text: row.get(13)?,
        updated_at: row.get(14)?,
        started_at: row.get(15)?,
        completed_at: row.get(16)?,
    })
}

fn normalize_media_types(values: &[String]) -> Vec<String> {
    let allowed = [
        "anime",
        "manga",
        "screenshot",
        "photo",
        "document",
        "artwork",
        "other",
    ];
    let mut out = Vec::new();
    for value in values {
        let normalized = value.trim().to_lowercase();
        if allowed.contains(&normalized.as_str()) && !out.contains(&normalized) {
            out.push(normalized);
        }
    }
    out
}

fn parse_date_start_ns(value: &str) -> Option<i64> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let dt = NaiveDateTime::new(date, NaiveTime::MIN);
    Utc.from_utc_datetime(&dt).timestamp_nanos_opt()
}

fn parse_date_end_ns(value: &str) -> Option<i64> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let dt = NaiveDateTime::new(date, NaiveTime::from_hms_opt(23, 59, 59)?);
    Utc.from_utc_datetime(&dt).timestamp_nanos_opt()
}

fn to_fts_query(input: &str) -> String {
    let tokens = input
        .split_whitespace()
        .map(sanitize_fts_token)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return String::new();
    }
    // Use OR to keep broad natural-language queries from over-filtering sparse metadata.
    tokens
        .into_iter()
        .map(|token| format!("{token}*"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn sanitize_fts_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>()
        .to_lowercase()
}

fn ensure_fts_schema(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(files_fts)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    let has_rel_path = columns.iter().any(|c| c == "rel_path");

    if !has_rel_path {
        conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS files_fts;
            CREATE VIRTUAL TABLE files_fts USING fts5(
                filename,
                rel_path,
                description,
                extracted_text,
                canonical_mentions
            );
            INSERT INTO files_fts (rowid, filename, rel_path, description, extracted_text, canonical_mentions)
            SELECT id, filename, rel_path, description, extracted_text, canonical_mentions
            FROM files;
            "#,
        )?;
    }
    Ok(())
}

fn rebuild_fts_with_location(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS files_fts;
        CREATE VIRTUAL TABLE files_fts USING fts5(
            filename,
            rel_path,
            description,
            extracted_text,
            canonical_mentions,
            location_text
        );
        INSERT INTO files_fts (rowid, filename, rel_path, description, extracted_text,
                               canonical_mentions, location_text)
        SELECT id, filename, rel_path, description, extracted_text,
               canonical_mentions, location_text FROM files;
        "#,
    )?;
    Ok(())
}

fn truncate_text(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect()
}

fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now_epoch_secs_pub() -> i64 {
    now_epoch_secs()
}

fn now_epoch_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Albums ──────────────────────────────────────────────────────────

pub fn create_album(db_path: &Path, name: &str) -> AppResult<Album> {
    let conn = open_conn(db_path)?;
    let now = now_epoch_secs();
    conn.execute(
        "INSERT INTO albums (name, created_at) VALUES (?1, ?2)",
        params![name, now],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Album {
        id,
        name: name.to_string(),
        created_at: now,
        file_count: 0,
    })
}

pub fn delete_album(db_path: &Path, album_id: i64) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute("DELETE FROM albums WHERE id = ?1", params![album_id])?;
    Ok(())
}

pub fn list_albums(db_path: &Path) -> AppResult<Vec<Album>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.created_at,
                (SELECT COUNT(*) FROM album_files af
                 JOIN files f ON f.id = af.file_id
                 WHERE af.album_id = a.id AND f.deleted_at IS NULL) AS file_count
         FROM albums a ORDER BY a.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Album {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            file_count: row.get::<_, i64>(3)? as u64,
        })
    })?;
    let mut albums = Vec::new();
    for row in rows {
        albums.push(row?);
    }
    Ok(albums)
}

pub fn add_files_to_album(db_path: &Path, album_id: i64, file_ids: &[i64]) -> AppResult<u64> {
    let conn = open_conn(db_path)?;
    let now = now_epoch_secs();
    let mut count = 0u64;
    for fid in file_ids {
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO album_files (album_id, file_id, added_at) VALUES (?1, ?2, ?3)",
            params![album_id, fid, now],
        )?;
        count += inserted as u64;
    }
    Ok(count)
}

pub fn remove_files_from_album(
    db_path: &Path,
    album_id: i64,
    file_ids: &[i64],
) -> AppResult<u64> {
    let conn = open_conn(db_path)?;
    let mut count = 0u64;
    for fid in file_ids {
        let deleted = conn.execute(
            "DELETE FROM album_files WHERE album_id = ?1 AND file_id = ?2",
            params![album_id, fid],
        )?;
        count += deleted as u64;
    }
    Ok(count)
}

// ── Smart Folders ───────────────────────────────────────────────────

pub fn create_smart_folder(db_path: &Path, name: &str, query: &str) -> AppResult<SmartFolder> {
    let conn = open_conn(db_path)?;
    let now = now_epoch_secs();
    conn.execute(
        "INSERT INTO smart_folders (name, query, created_at) VALUES (?1, ?2, ?3)",
        params![name, query, now],
    )?;
    let id = conn.last_insert_rowid();
    Ok(SmartFolder {
        id,
        name: name.to_string(),
        query: query.to_string(),
        created_at: now,
    })
}

pub fn delete_smart_folder(db_path: &Path, folder_id: i64) -> AppResult<()> {
    let conn = open_conn(db_path)?;
    conn.execute(
        "DELETE FROM smart_folders WHERE id = ?1",
        params![folder_id],
    )?;
    Ok(())
}

pub fn list_smart_folders(db_path: &Path) -> AppResult<Vec<SmartFolder>> {
    let conn = open_conn(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, query, created_at FROM smart_folders ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SmartFolder {
            id: row.get(0)?,
            name: row.get(1)?,
            query: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    let mut folders = Vec::new();
    for row in rows {
        folders.push(row?);
    }
    Ok(folders)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::models::FileRecordUpsert;

    fn test_db_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.sqlite");
        (dir, db_path)
    }

    fn sample_record(root_id: i64, rel: &str, fp: &str) -> FileRecordUpsert {
        FileRecordUpsert {
            root_id,
            rel_path: rel.to_string(),
            abs_path: format!("/tmp/demo/{rel}"),
            filename: crate::platform::paths::rel_path_filename(rel).to_string(),
            media_type: "photo".to_string(),
            description: format!("desc of {rel}"),
            extracted_text: String::new(),
            canonical_mentions: String::new(),
            confidence: 0.7,
            lang_hint: "en".to_string(),
            mtime_ns: 1_700_000_000_000_000_000,
            size_bytes: 10_000,
            fingerprint: fp.to_string(),
            scan_marker: 123,
            location_text: String::new(),
        }
    }

    #[test]
    fn creates_schema_and_stats() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let stats = database_stats(&db_path).expect("stats");
        assert_eq!(stats.roots, 0);
        assert_eq!(stats.files, 0);
    }

    #[test]
    fn paginates_search_results() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        for i in 0..250 {
            let rec = FileRecordUpsert {
                root_id,
                rel_path: format!("images/{i}.jpg"),
                abs_path: format!("/tmp/demo/images/{i}.jpg"),
                filename: format!("{i}.jpg"),
                media_type: "photo".to_string(),
                description: format!("Demo image {i}"),
                extracted_text: String::new(),
                canonical_mentions: String::new(),
                confidence: 0.7,
                lang_hint: "en".to_string(),
                mtime_ns: 1_700_000_000_000_000_000 + i,
                size_bytes: 10_000,
                fingerprint: format!("fp-{i}"),
                scan_marker: 123,
                location_text: String::new(),
            };
            upsert_file_record(&db_path, &rec).expect("upsert");
        }

        let req = SearchRequest {
            query: "".to_string(),
            limit: Some(120),
            offset: Some(0),
            ..SearchRequest::default()
        };
        let page1 = search_images(&db_path, &req).expect("page1");
        assert_eq!(page1.items.len(), 120);
        assert_eq!(page1.total, 250);

        let req2 = SearchRequest {
            offset: Some(120),
            ..req
        };
        let page2 = search_images(&db_path, &req2).expect("page2");
        assert_eq!(page2.items.len(), 120);
    }

    #[test]
    fn fts_matches_description() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let rec = FileRecordUpsert {
            root_id,
            rel_path: "images/ranma.jpg".to_string(),
            abs_path: "/tmp/demo/images/ranma.jpg".to_string(),
            filename: "ranma.jpg".to_string(),
            media_type: "anime".to_string(),
            description: "Ranma from Ranma 1/2 series".to_string(),
            extracted_text: String::new(),
            canonical_mentions: "Ranma Saotome".to_string(),
            confidence: 0.9,
            lang_hint: "en".to_string(),
            mtime_ns: 1_700_000_000_000_000_000,
            size_bytes: 10_000,
            fingerprint: "fp-1".to_string(),
            scan_marker: 123,
            location_text: String::new(),
        };
        upsert_file_record(&db_path, &rec).expect("upsert");

        let req = SearchRequest {
            query: "Ranma".to_string(),
            limit: Some(20),
            offset: Some(0),
            ..SearchRequest::default()
        };
        let result = search_images(&db_path, &req).expect("search");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].media_type, "anime");
    }

    #[test]
    fn falls_back_when_parser_filters_are_too_strict() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let rec = FileRecordUpsert {
            root_id,
            rel_path: "images/ranma.jpg".to_string(),
            abs_path: "/tmp/demo/images/ranma.jpg".to_string(),
            filename: "ranma.jpg".to_string(),
            media_type: "other".to_string(),
            description: "Character poster".to_string(),
            extracted_text: String::new(),
            canonical_mentions: String::new(),
            confidence: 0.0,
            lang_hint: "unknown".to_string(),
            mtime_ns: 1_700_000_000_000_000_000,
            size_bytes: 10_000,
            fingerprint: "fp-2".to_string(),
            scan_marker: 123,
            location_text: String::new(),
        };
        upsert_file_record(&db_path, &rec).expect("upsert");

        // Query parser infers media_type=anime, which would otherwise hide this file.
        let req = SearchRequest {
            query: "anime ranma".to_string(),
            limit: Some(20),
            offset: Some(0),
            ..SearchRequest::default()
        };
        let result = search_images(&db_path, &req).expect("search");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].rel_path, "images/ranma.jpg");
    }

    #[test]
    fn touch_scan_marker_preserves_classification() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let rec = FileRecordUpsert {
            root_id,
            rel_path: "a.jpg".to_string(),
            abs_path: "/tmp/demo/a.jpg".to_string(),
            filename: "a.jpg".to_string(),
            media_type: "anime".to_string(),
            description: "test desc".to_string(),
            extracted_text: "ocr text".to_string(),
            canonical_mentions: "Ranma".to_string(),
            confidence: 0.85,
            lang_hint: "en".to_string(),
            mtime_ns: 100,
            size_bytes: 200,
            fingerprint: "fp1".to_string(),
            scan_marker: 1,
            location_text: String::new(),
        };
        upsert_file_record(&db_path, &rec).expect("upsert");
        touch_file_scan_marker(&db_path, root_id, "a.jpg", 2).expect("touch");

        let conn = open_conn(&db_path).expect("open");
        let (media_type, conf, marker): (String, f64, i64) = conn
            .query_row(
                "SELECT media_type, confidence, scan_marker FROM files WHERE root_id = ?1 AND rel_path = ?2",
                params![root_id, "a.jpg"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query");
        assert_eq!(media_type, "anime");
        assert!((conf - 0.85).abs() < 0.01);
        assert_eq!(marker, 2);
    }

    #[test]
    fn load_existing_files_includes_mtime_size() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let rec = FileRecordUpsert {
            root_id,
            rel_path: "b.jpg".to_string(),
            abs_path: "/tmp/demo/b.jpg".to_string(),
            filename: "b.jpg".to_string(),
            media_type: "photo".to_string(),
            description: String::new(),
            extracted_text: String::new(),
            canonical_mentions: String::new(),
            confidence: 0.0,
            lang_hint: "unknown".to_string(),
            mtime_ns: 999,
            size_bytes: 5000,
            fingerprint: "fp-x".to_string(),
            scan_marker: 10,
            location_text: String::new(),
        };
        upsert_file_record(&db_path, &rec).expect("upsert");
        let files = load_existing_files(&db_path, root_id).expect("load");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].mtime_ns, 999);
        assert_eq!(files[0].size_bytes, 5000);
    }

    #[test]
    fn get_deleted_file_paths_returns_deleted() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let rec = FileRecordUpsert {
            root_id,
            rel_path: "del.jpg".to_string(),
            abs_path: "/tmp/demo/del.jpg".to_string(),
            filename: "del.jpg".to_string(),
            media_type: "other".to_string(),
            description: String::new(),
            extracted_text: String::new(),
            canonical_mentions: String::new(),
            confidence: 0.0,
            lang_hint: "unknown".to_string(),
            mtime_ns: 100,
            size_bytes: 100,
            fingerprint: "fp-del".to_string(),
            scan_marker: 5,
            location_text: String::new(),
        };
        upsert_file_record(&db_path, &rec).expect("upsert");
        mark_missing_as_deleted(&db_path, root_id, 99).expect("delete");

        let conn = open_conn(&db_path).expect("open");
        let deleted_at: i64 = conn
            .query_row(
                "SELECT deleted_at FROM files WHERE root_id = ?1 AND rel_path = 'del.jpg'",
                params![root_id],
                |r| r.get(0),
            )
            .expect("q");

        let paths = get_deleted_file_paths(&db_path, root_id, deleted_at).expect("get");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, "del.jpg");
    }

    #[test]
    fn creates_and_recovers_scan_jobs() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        let job = create_or_resume_scan_job(&db_path, "/tmp/demo").expect("job");
        assert_eq!(job.status, "running");
        checkpoint_scan_job(&db_path, job.id, 20, 7, Some("a.jpg"), 1, 2, 0, 4).expect("ckpt");
        fail_scan_job(&db_path, job.id, "failure").expect("fail");

        let resumed = create_or_resume_scan_job(&db_path, "/tmp/demo").expect("resume");
        assert_eq!(resumed.id, job.id);
        assert_eq!(resumed.status, "running");

        let changed = recover_incomplete_scan_jobs(&db_path).expect("recover");
        assert!(changed >= 1);
    }

    // -----------------------------------------------------------------------
    // New tests: Phase 8
    // -----------------------------------------------------------------------

    #[test]
    fn test_open_conn_sets_pragmas() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let conn = open_conn(&db_path).expect("open");

        let timeout: i64 = conn
            .pragma_query_value(None, "busy_timeout", |r| r.get(0))
            .expect("busy_timeout");
        assert_eq!(timeout, 5000);

        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .expect("foreign_keys");
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_purge_root_cascades() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/purge_test").expect("root");

        // Insert files
        for i in 0..3 {
            upsert_file_record(
                &db_path,
                &sample_record(root_id, &format!("f{i}.jpg"), &format!("fp{i}")),
            )
            .expect("upsert");
        }
        // Create a scan job
        create_or_resume_scan_job(&db_path, "/tmp/purge_test").expect("job");

        // Verify data exists
        let conn = open_conn(&db_path).expect("open");
        let file_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE root_id = ?1",
                params![root_id],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(file_count, 3);
        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
            .expect("fts count");
        assert!(fts_count >= 3);
        drop(conn);

        // Purge
        let result = purge_root(&db_path, root_id).expect("purge");
        assert_eq!(result.files_removed, 3);
        assert!(result.jobs_removed >= 1);

        // Verify everything is gone
        let conn = open_conn(&db_path).expect("open");
        let root_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM roots WHERE id = ?1",
                params![root_id],
                |r| r.get(0),
            )
            .expect("root count");
        assert_eq!(root_count, 0);
        let file_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE root_id = ?1",
                params![root_id],
                |r| r.get(0),
            )
            .expect("file count");
        assert_eq!(file_count, 0);
        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
            .expect("fts count");
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn test_mark_missing_cleans_fts() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        // Insert a file with scan_marker=1
        let rec = sample_record(root_id, "gone.jpg", "fp-gone");
        upsert_file_record(
            &db_path,
            &FileRecordUpsert {
                scan_marker: 1,
                ..rec
            },
        )
        .expect("upsert");

        // Verify FTS entry exists
        let conn = open_conn(&db_path).expect("open");
        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
            .expect("fts");
        assert_eq!(fts_count, 1);
        drop(conn);

        // Mark missing with a different scan_marker
        let deleted = mark_missing_as_deleted(&db_path, root_id, 999).expect("mark");
        assert_eq!(deleted, 1);

        // Verify FTS entry is gone
        let conn = open_conn(&db_path).expect("open");
        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
            .expect("fts");
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn test_quick_check_healthy_db() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        assert!(quick_check(&db_path).expect("check"));
    }

    #[test]
    fn test_backup_and_restore() {
        let (dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/backup_test").expect("root");
        upsert_file_record(&db_path, &sample_record(root_id, "img.jpg", "fp-img")).expect("upsert");

        // Backup
        let backup_path = dir.path().join("backup.sqlite");
        backup_database(&db_path, &backup_path).expect("backup");

        // Corrupt original by truncating
        std::fs::write(&db_path, b"corrupted").expect("corrupt");

        // Restore
        restore_from_backup(&backup_path, &db_path).expect("restore");

        // Verify data is intact
        let stats = database_stats(&db_path).expect("stats");
        assert_eq!(stats.roots, 1);
        assert_eq!(stats.files, 1);
    }

    #[test]
    fn test_recreate_database() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        upsert_root(&db_path, "/tmp/recreate").expect("root");

        recreate_database(&db_path).expect("recreate");

        let stats = database_stats(&db_path).expect("stats");
        assert_eq!(stats.roots, 0);
        assert_eq!(stats.files, 0);
        // Schema should still be valid
        assert!(quick_check(&db_path).expect("check"));
    }

    #[test]
    fn test_startup_health_check_healthy() {
        let (dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let outcome = startup_health_check(&db_path, dir.path()).expect("health");
        assert!(matches!(outcome, HealthCheckOutcome::Healthy));
    }

    #[test]
    fn test_validate_and_purge_stale_roots() {
        let (dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        // Insert root for a non-existent directory
        let conn = open_conn(&db_path).expect("open");
        conn.execute(
            "INSERT INTO roots(root_path, root_name, created_at) VALUES (?1, ?2, ?3)",
            params![
                "/nonexistent/path/that/does/not/exist",
                "ghost",
                now_epoch_secs()
            ],
        )
        .expect("insert root");
        drop(conn);

        let thumbs_dir = dir.path().join("thumbs");
        std::fs::create_dir_all(&thumbs_dir).expect("thumbs dir");

        let purged = validate_and_purge_stale_roots(&db_path, &thumbs_dir).expect("validate");
        assert_eq!(purged.len(), 1);
        assert!(purged[0].contains("nonexistent"));

        // Root should be gone
        let roots = list_roots(&db_path).expect("list");
        assert!(roots.is_empty());
    }

    #[test]
    fn test_upsert_file_record_transactional() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let rec = sample_record(root_id, "atomic.jpg", "fp-atomic");
        let file_id = upsert_file_record(&db_path, &rec).expect("upsert");

        // Verify both file record and FTS entry exist atomically
        let conn = open_conn(&db_path).expect("open");
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM files WHERE id = ?1)",
                params![file_id],
                |r| r.get(0),
            )
            .expect("file exists");
        assert!(exists);

        let fts_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM files_fts WHERE rowid = ?1)",
                params![file_id],
                |r| r.get(0),
            )
            .expect("fts exists");
        assert!(fts_exists);
    }

    #[test]
    fn test_list_roots() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        upsert_root(&db_path, "/tmp/root_a").expect("root_a");
        upsert_root(&db_path, "/tmp/root_b").expect("root_b");
        upsert_root(&db_path, "/tmp/root_c").expect("root_c");

        let roots = list_roots(&db_path).expect("list");
        assert_eq!(roots.len(), 3);
        let paths: Vec<&str> = roots.iter().map(|r| r.root_path.as_str()).collect();
        assert!(paths.contains(&"/tmp/root_a"));
        assert!(paths.contains(&"/tmp/root_b"));
        assert!(paths.contains(&"/tmp/root_c"));
    }

    /// Prepare a DB for read-only testing: checkpoint WAL, switch to DELETE
    /// journal mode, close all connections, remove WAL/SHM files, then make
    /// the containing directory read-only.
    #[cfg(unix)]
    fn make_db_readonly(dir: &std::path::Path, db_path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        // Checkpoint and switch out of WAL mode so no WAL/SHM files are needed
        let conn = Connection::open(db_path).expect("open for journal switch");
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .expect("checkpoint");
        conn.pragma_update(None, "journal_mode", "DELETE")
            .expect("journal_mode");
        drop(conn);
        // Remove WAL/SHM files
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
        // Make directory read-only
        let mut perms = std::fs::metadata(dir).expect("meta").permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(dir, perms).expect("chmod");
    }

    #[cfg(unix)]
    fn restore_dir_writable(dir: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dir).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(dir, perms).expect("restore chmod");
    }

    #[test]
    #[cfg(unix)]
    fn test_open_conn_ro_fallback() {
        let (dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/ro_test").expect("root");
        upsert_file_record(&db_path, &sample_record(root_id, "ro.jpg", "fp-ro")).expect("upsert");

        make_db_readonly(dir.path(), &db_path);

        // open_conn should fall back to RO and succeed
        let conn = open_conn(&db_path).expect("open_conn RO fallback");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
            .expect("select");
        assert_eq!(count, 1);

        restore_dir_writable(dir.path());
    }

    #[test]
    #[cfg(unix)]
    fn test_database_stats_on_readonly() {
        let (dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/ro_stats").expect("root");
        for i in 0..3 {
            upsert_file_record(
                &db_path,
                &sample_record(root_id, &format!("f{i}.jpg"), &format!("fp-ro-{i}")),
            )
            .expect("upsert");
        }

        make_db_readonly(dir.path(), &db_path);

        let stats = database_stats(&db_path).expect("stats RO");
        assert_eq!(stats.roots, 1);
        assert_eq!(stats.files, 3);

        restore_dir_writable(dir.path());
    }

    // -----------------------------------------------------------------------
    // Sort order tests
    // -----------------------------------------------------------------------

    fn insert_sort_test_files(db_path: &std::path::Path, root_id: i64) {
        let files = vec![
            ("alpha.jpg", "photo", 3000_i64),
            ("charlie.jpg", "anime", 1000),
            ("bravo.jpg", "document", 2000),
        ];
        for (name, media, mtime) in files {
            let rec = FileRecordUpsert {
                root_id,
                rel_path: name.to_string(),
                abs_path: format!("/tmp/sort/{name}"),
                filename: name.to_string(),
                media_type: media.to_string(),
                description: format!("desc {name}"),
                extracted_text: String::new(),
                canonical_mentions: String::new(),
                confidence: 0.7,
                lang_hint: "en".to_string(),
                mtime_ns: mtime,
                size_bytes: 100,
                fingerprint: format!("fp-{name}"),
                scan_marker: 1,
                location_text: String::new(),
            };
            upsert_file_record(db_path, &rec).expect("upsert");
        }
    }

    #[test]
    fn sort_by_name_asc() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/sort").expect("root");
        insert_sort_test_files(&db_path, root_id);

        let req = SearchRequest {
            sort_by: SortField::Name,
            sort_order: SortOrder::Asc,
            ..SearchRequest::default()
        };
        let res = search_images(&db_path, &req).expect("search");
        let names: Vec<&str> = res.items.iter().map(|i| i.rel_path.as_str()).collect();
        assert_eq!(names, vec!["alpha.jpg", "bravo.jpg", "charlie.jpg"]);
    }

    #[test]
    fn sort_by_date_desc() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/sort").expect("root");
        insert_sort_test_files(&db_path, root_id);

        let req = SearchRequest {
            sort_by: SortField::DateModified,
            sort_order: SortOrder::Desc,
            ..SearchRequest::default()
        };
        let res = search_images(&db_path, &req).expect("search");
        let names: Vec<&str> = res.items.iter().map(|i| i.rel_path.as_str()).collect();
        assert_eq!(names, vec!["alpha.jpg", "bravo.jpg", "charlie.jpg"]);
    }

    #[test]
    fn sort_by_type_asc() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/sort").expect("root");
        insert_sort_test_files(&db_path, root_id);

        let req = SearchRequest {
            sort_by: SortField::Type,
            sort_order: SortOrder::Asc,
            ..SearchRequest::default()
        };
        let res = search_images(&db_path, &req).expect("search");
        let types: Vec<&str> = res.items.iter().map(|i| i.media_type.as_str()).collect();
        assert_eq!(types, vec!["anime", "document", "photo"]);
    }

    // -----------------------------------------------------------------------
    // Migration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_migrations_apply_to_fresh_db() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        let conn = open_conn(&db_path).expect("open");
        // Verify user_version is set (5 migrations applied → version 5)
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .expect("user_version");
        assert_eq!(version, 5);

        // Verify all tables exist
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .expect("prepare");
            stmt.query_map([], |r| r.get(0))
                .expect("query")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect")
        };
        assert!(tables.contains(&"roots".to_string()));
        assert!(tables.contains(&"files".to_string()));
        assert!(tables.contains(&"scan_jobs".to_string()));
        assert!(tables.contains(&"files_fts".to_string()));
        assert!(tables.contains(&"albums".to_string()));
        assert!(tables.contains(&"album_files".to_string()));
        assert!(tables.contains(&"smart_folders".to_string()));
    }

    #[test]
    fn test_migrations_idempotent() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("first init");
        init_database(&db_path).expect("second init should be no-op");

        let stats = database_stats(&db_path).expect("stats");
        assert_eq!(stats.roots, 0);
        assert_eq!(stats.files, 0);
    }

    #[test]
    fn test_schema_version_set() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        let conn = open_conn(&db_path).expect("open");
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .expect("user_version");
        // We have 5 migrations (indices 0..4), so user_version should be 5
        assert_eq!(version, 5);
    }

    #[test]
    fn sort_relevance_with_query() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/sort").expect("root");
        insert_sort_test_files(&db_path, root_id);

        let req = SearchRequest {
            query: "alpha".to_string(),
            sort_by: SortField::Relevance,
            sort_order: SortOrder::Desc,
            ..SearchRequest::default()
        };
        let res = search_images(&db_path, &req).expect("search");
        assert!(res.total >= 1);
        assert_eq!(res.items[0].rel_path, "alpha.jpg");
    }

    // -----------------------------------------------------------------------
    // File delete / rename tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_collect_and_delete_files() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let id1 =
            upsert_file_record(&db_path, &sample_record(root_id, "a.jpg", "fp-a")).expect("upsert");
        let id2 =
            upsert_file_record(&db_path, &sample_record(root_id, "b.jpg", "fp-b")).expect("upsert");
        let _id3 =
            upsert_file_record(&db_path, &sample_record(root_id, "c.jpg", "fp-c")).expect("upsert");

        // Phase 1: collect — DB is not mutated
        let infos = collect_files_for_delete(&db_path, &[id1, id2]).expect("collect");
        assert_eq!(infos.len(), 2);
        assert_eq!(database_stats(&db_path).expect("stats").files, 3); // still 3

        // Phase 2: delete records
        let ids: Vec<i64> = infos.iter().map(|(fid, _, _)| *fid).collect();
        delete_file_records(&db_path, &ids).expect("delete");

        // Verify files are gone from DB
        assert_eq!(database_stats(&db_path).expect("stats").files, 1);

        // Verify FTS entries are cleaned up
        let conn = open_conn(&db_path).expect("open");
        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files_fts", [], |r| r.get(0))
            .expect("fts count");
        assert_eq!(fts_count, 1);
    }

    #[test]
    fn test_collect_files_nonexistent() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let infos = collect_files_for_delete(&db_path, &[999, 1000]).expect("collect");
        assert_eq!(infos.len(), 0);
    }

    #[test]
    fn test_delete_file_records_empty() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        // Deleting zero ids should be a no-op
        delete_file_records(&db_path, &[]).expect("delete empty");
    }

    #[test]
    fn test_partial_delete_preserves_remaining() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let id1 =
            upsert_file_record(&db_path, &sample_record(root_id, "a.jpg", "fp-a")).expect("upsert");
        let id2 =
            upsert_file_record(&db_path, &sample_record(root_id, "b.jpg", "fp-b")).expect("upsert");

        // Only delete one — simulates partial filesystem success
        delete_file_records(&db_path, &[id1]).expect("delete");
        assert_eq!(database_stats(&db_path).expect("stats").files, 1);

        // The remaining file should still be accessible
        let (abs, _, _) = get_file_path_info(&db_path, id2).expect("info");
        assert_eq!(abs, "/tmp/demo/b.jpg");
    }

    #[test]
    fn test_get_file_path_info() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let file_id = upsert_file_record(&db_path, &sample_record(root_id, "info.jpg", "fp-info"))
            .expect("upsert");

        let (abs_path, rel_path, thumb_path) = get_file_path_info(&db_path, file_id).expect("info");
        assert_eq!(abs_path, "/tmp/demo/info.jpg");
        assert_eq!(rel_path, "info.jpg");
        assert!(thumb_path.is_none());
    }

    #[test]
    fn test_rename_file_record() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let file_id = upsert_file_record(
            &db_path,
            &sample_record(root_id, "old_name.jpg", "fp-rename"),
        )
        .expect("upsert");

        rename_file_record(
            &db_path,
            file_id,
            "new_name.jpg",
            "/tmp/demo/new_name.jpg",
            "new_name.jpg",
        )
        .expect("rename");

        // Verify DB update
        let (abs_path, rel_path, _) = get_file_path_info(&db_path, file_id).expect("info");
        assert_eq!(rel_path, "new_name.jpg");
        assert_eq!(abs_path, "/tmp/demo/new_name.jpg");

        // Verify FTS is updated (search by new name should work)
        let req = SearchRequest {
            query: "new_name".to_string(),
            ..SearchRequest::default()
        };
        let res = search_images(&db_path, &req).expect("search");
        assert_eq!(res.total, 1);
        assert_eq!(res.items[0].rel_path, "new_name.jpg");
    }

    // -----------------------------------------------------------------------
    // Location text / metadata tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_migration_adds_location_text_column() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        let conn = open_conn(&db_path).expect("open");
        // Verify location_text column exists in files table
        let mut stmt = conn.prepare("PRAGMA table_info(files)").expect("prepare");
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect");
        assert!(columns.contains(&"location_text".to_string()));

        // Verify FTS5 has 6 columns (including location_text)
        let mut fts_stmt = conn
            .prepare("PRAGMA table_info(files_fts)")
            .expect("fts prepare");
        let fts_columns: Vec<String> = fts_stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect");
        assert_eq!(fts_columns.len(), 6);
        assert!(fts_columns.contains(&"location_text".to_string()));
    }

    #[test]
    fn test_upsert_with_location_text() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let mut rec = sample_record(root_id, "nyc.jpg", "fp-nyc");
        rec.location_text = "New York, New York, US".to_string();
        upsert_file_record(&db_path, &rec).expect("upsert");

        let conn = open_conn(&db_path).expect("open");
        let location: String = conn
            .query_row(
                "SELECT location_text FROM files WHERE root_id = ?1 AND rel_path = 'nyc.jpg'",
                params![root_id],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(location, "New York, New York, US");
    }

    #[test]
    fn test_fts_matches_location_text() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let mut rec = sample_record(root_id, "tokyo.jpg", "fp-tokyo");
        rec.location_text = "Tokyo, Kanto, JP".to_string();
        upsert_file_record(&db_path, &rec).expect("upsert");

        let req = SearchRequest {
            query: "Tokyo".to_string(),
            ..SearchRequest::default()
        };
        let result = search_images(&db_path, &req).expect("search");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].rel_path, "tokyo.jpg");
    }

    #[test]
    fn test_get_file_metadata() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let mut rec = sample_record(root_id, "meta.jpg", "fp-meta");
        rec.media_type = "anime".to_string();
        rec.description = "Test description".to_string();
        rec.extracted_text = "OCR text here".to_string();
        rec.canonical_mentions = "Character A, Character B".to_string();
        rec.location_text = "Paris, Ile-de-France, FR".to_string();
        let file_id = upsert_file_record(&db_path, &rec).expect("upsert");

        let meta = get_file_metadata(&db_path, file_id).expect("get");
        assert_eq!(meta.id, file_id);
        assert_eq!(meta.media_type, "anime");
        assert_eq!(meta.description, "Test description");
        assert_eq!(meta.extracted_text, "OCR text here");
        assert_eq!(meta.canonical_mentions, "Character A, Character B");
        assert_eq!(meta.location_text, "Paris, Ile-de-France, FR");
    }

    #[test]
    fn test_update_file_metadata() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");

        let rec = sample_record(root_id, "edit.jpg", "fp-edit");
        let file_id = upsert_file_record(&db_path, &rec).expect("upsert");

        update_file_metadata(
            &db_path,
            file_id,
            "document",
            "Updated description",
            "New OCR text",
            "Alice, Bob",
            "London, England, GB",
        )
        .expect("update");

        let meta = get_file_metadata(&db_path, file_id).expect("get");
        assert_eq!(meta.media_type, "document");
        assert_eq!(meta.description, "Updated description");
        assert_eq!(meta.extracted_text, "New OCR text");
        assert_eq!(meta.canonical_mentions, "Alice, Bob");
        assert_eq!(meta.location_text, "London, England, GB");

        // Verify FTS is updated — search by new location
        let req = SearchRequest {
            query: "London".to_string(),
            ..SearchRequest::default()
        };
        let result = search_images(&db_path, &req).expect("search");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].rel_path, "edit.jpg");
    }

    // -----------------------------------------------------------------------
    // Album CRUD tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_album_crud() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        let album = create_album(&db_path, "Vacation").expect("create");
        assert_eq!(album.name, "Vacation");
        assert_eq!(album.file_count, 0);

        let albums = list_albums(&db_path).expect("list");
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].name, "Vacation");

        delete_album(&db_path, album.id).expect("delete");
        let albums = list_albums(&db_path).expect("list after delete");
        assert!(albums.is_empty());
    }

    #[test]
    fn test_album_duplicate_name_fails() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        create_album(&db_path, "Travel").expect("create first");
        let result = create_album(&db_path, "Travel");
        assert!(result.is_err());
    }

    #[test]
    fn test_album_membership() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let f1 = upsert_file_record(&db_path, &sample_record(root_id, "a.jpg", "fp-a"))
            .expect("upsert");
        let f2 = upsert_file_record(&db_path, &sample_record(root_id, "b.jpg", "fp-b"))
            .expect("upsert");

        let album = create_album(&db_path, "Best").expect("create");
        let added = add_files_to_album(&db_path, album.id, &[f1, f2]).expect("add");
        assert_eq!(added, 2);

        // list_albums should reflect file_count
        let albums = list_albums(&db_path).expect("list");
        assert_eq!(albums[0].file_count, 2);

        // INSERT OR IGNORE — adding the same file again should be a no-op
        let added_again = add_files_to_album(&db_path, album.id, &[f1]).expect("add again");
        assert_eq!(added_again, 0);

        // Remove one file
        let removed = remove_files_from_album(&db_path, album.id, &[f1]).expect("remove");
        assert_eq!(removed, 1);
        let albums = list_albums(&db_path).expect("list after remove");
        assert_eq!(albums[0].file_count, 1);
    }

    #[test]
    fn test_search_with_album_filter() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");
        let root_id = upsert_root(&db_path, "/tmp/demo").expect("root");
        let f1 = upsert_file_record(&db_path, &sample_record(root_id, "in_album.jpg", "fp-1"))
            .expect("upsert");
        let _f2 = upsert_file_record(&db_path, &sample_record(root_id, "not_in_album.jpg", "fp-2"))
            .expect("upsert");

        let album = create_album(&db_path, "MyAlbum").expect("create");
        add_files_to_album(&db_path, album.id, &[f1]).expect("add");

        // Search with album: prefix
        let req = SearchRequest {
            query: "album:MyAlbum".to_string(),
            ..SearchRequest::default()
        };
        let res = search_images(&db_path, &req).expect("search");
        assert_eq!(res.total, 1);
        assert_eq!(res.items[0].rel_path, "in_album.jpg");
    }

    // -----------------------------------------------------------------------
    // Smart folder CRUD tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_smart_folder_crud() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        let sf = create_smart_folder(&db_path, "Anime photos", "anime photo").expect("create");
        assert_eq!(sf.name, "Anime photos");
        assert_eq!(sf.query, "anime photo");

        let folders = list_smart_folders(&db_path).expect("list");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Anime photos");

        delete_smart_folder(&db_path, sf.id).expect("delete");
        let folders = list_smart_folders(&db_path).expect("list after delete");
        assert!(folders.is_empty());
    }

    #[test]
    fn test_smart_folder_duplicate_name_fails() {
        let (_dir, db_path) = test_db_path();
        init_database(&db_path).expect("init");

        create_smart_folder(&db_path, "Receipts", "document receipt").expect("create");
        let result = create_smart_folder(&db_path, "Receipts", "different query");
        assert!(result.is_err());
    }
}
