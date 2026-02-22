use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rusqlite::{params, params_from_iter, types::Value, Connection};

use crate::error::AppResult;
use crate::models::{
    DbStats, ExistingFile, FileRecordUpsert, ParsedQuery, SearchItem, SearchRequest, SearchResponse,
};
use crate::query_parser::parse_query;

const DEFAULT_LIMIT: u32 = 80;
const MAX_LIMIT: u32 = 200;

pub fn init_database(db_path: &Path) -> AppResult<()> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.execute_batch(
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

        CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
            filename,
            description,
            extracted_text,
            canonical_mentions
        );
        "#,
    )?;
    Ok(())
}

pub fn database_stats(db_path: &Path) -> AppResult<DbStats> {
    let conn = Connection::open(db_path)?;
    let roots: i64 = conn.query_row("SELECT COUNT(*) FROM roots", [], |r| r.get(0))?;
    let files: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(DbStats {
        roots: roots as u64,
        files: files as u64,
    })
}

pub fn upsert_root(db_path: &Path, root_path: &str) -> AppResult<i64> {
    let conn = Connection::open(db_path)?;
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
    let conn = Connection::open(db_path)?;
    conn.execute(
        "UPDATE roots SET last_scan_at = ?2 WHERE id = ?1",
        params![root_id, now_epoch_secs()],
    )?;
    Ok(())
}

pub fn load_existing_files(db_path: &Path, root_id: i64) -> AppResult<Vec<ExistingFile>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, fingerprint
         FROM files
         WHERE root_id = ?1 AND deleted_at IS NULL",
    )?;

    let rows = stmt.query_map(params![root_id], |row| {
        Ok(ExistingFile {
            id: row.get(0)?,
            rel_path: row.get(1)?,
            fingerprint: row.get(2)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn upsert_file_record(db_path: &Path, record: &FileRecordUpsert) -> AppResult<i64> {
    let conn = Connection::open(db_path)?;
    let now = now_epoch_secs();
    conn.execute(
        r#"
        INSERT INTO files (
            root_id, rel_path, filename, abs_path,
            media_type, description, extracted_text, canonical_mentions,
            confidence, lang_hint, mtime_ns, size_bytes, fingerprint,
            scan_marker, updated_at, deleted_at
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, NULL
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
            deleted_at = NULL
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
            now
        ],
    )?;

    let file_id: i64 = conn.query_row(
        "SELECT id FROM files WHERE root_id = ?1 AND rel_path = ?2",
        params![record.root_id, record.rel_path],
        |r| r.get(0),
    )?;
    refresh_fts(&conn, file_id)?;
    Ok(file_id)
}

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
    let conn = Connection::open(db_path)?;
    conn.execute(
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
    refresh_fts(&conn, file_id)?;
    Ok(())
}

pub fn mark_missing_as_deleted(db_path: &Path, root_id: i64, scan_marker: i64) -> AppResult<u64> {
    let conn = Connection::open(db_path)?;
    let affected = conn.execute(
        "UPDATE files
         SET deleted_at = ?3, updated_at = ?3
         WHERE root_id = ?1
           AND deleted_at IS NULL
           AND scan_marker <> ?2",
        params![root_id, scan_marker, now_epoch_secs()],
    )?;
    Ok(affected as u64)
}

fn refresh_fts(conn: &Connection, file_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM files_fts WHERE rowid = ?1", params![file_id])?;
    conn.execute(
        r#"
        INSERT INTO files_fts (rowid, filename, description, extracted_text, canonical_mentions)
        SELECT id, filename, description, extracted_text, canonical_mentions
        FROM files
        WHERE id = ?1
        "#,
        params![file_id],
    )?;
    Ok(())
}

pub fn search_images(db_path: &Path, request: &SearchRequest) -> AppResult<SearchResponse> {
    let parsed = parse_query(&request.query);
    let normalized = normalize_request(request, &parsed);
    search_images_normalized(db_path, normalized, parsed)
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
    let conn = Connection::open(db_path)?;
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = request.offset.unwrap_or(0);
    let query = request.query.trim();
    let has_query = !query.is_empty();

    let mut from_sql = String::from(" FROM files f ");
    let mut where_clauses = vec!["f.deleted_at IS NULL".to_string()];
    let mut bind_values: Vec<Value> = Vec::new();

    if has_query {
        from_sql.push_str(" JOIN files_fts ON files_fts.rowid = f.id ");
        where_clauses.push("files_fts MATCH ?".to_string());
        bind_values.push(Value::Text(to_fts_query(query)));
    }

    if !request.root_scope.is_empty() {
        let placeholders = vec!["?"; request.root_scope.len()].join(", ");
        where_clauses.push(format!("f.root_id IN ({placeholders})"));
        for root_id in &request.root_scope {
            bind_values.push(Value::Integer(*root_id));
        }
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

    let order_sql = if has_query {
        " ORDER BY bm25(files_fts), f.confidence DESC, f.updated_at DESC "
    } else {
        " ORDER BY f.updated_at DESC "
    };
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
        items.push(item?);
    }

    Ok(SearchResponse {
        total: total as u64,
        limit,
        offset,
        items,
        parsed_query: parsed,
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
    Some(Utc.from_utc_datetime(&dt).timestamp_nanos_opt()?)
}

fn parse_date_end_ns(value: &str) -> Option<i64> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let dt = NaiveDateTime::new(date, NaiveTime::from_hms_opt(23, 59, 59)?);
    Some(Utc.from_utc_datetime(&dt).timestamp_nanos_opt()?)
}

fn to_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| token.replace('"', ""))
        .filter(|token| !token.is_empty())
        .map(|token| format!(r#""{token}"*"#))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
}
