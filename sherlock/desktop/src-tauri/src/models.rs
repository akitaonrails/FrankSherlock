#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub status: String,
    pub mode: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStats {
    pub roots: u64,
    pub files: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub running_models: u64,
    pub stopped_models: u64,
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    Relevance,
    #[default]
    DateModified,
    Name,
    Type,
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
    #[serde(default)]
    pub root_scope: Vec<i64>,
    #[serde(default)]
    pub media_types: Vec<String>,
    #[serde(default)]
    pub min_confidence: Option<f32>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub sort_by: SortField,
    #[serde(default)]
    pub sort_order: SortOrder,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
    pub items: Vec<SearchItem>,
    pub parsed_query: ParsedQuery,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    pub id: i64,
    pub root_id: i64,
    pub rel_path: String,
    pub abs_path: String,
    pub media_type: String,
    pub description: String,
    pub confidence: f32,
    pub mtime_ns: i64,
    pub size_bytes: i64,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedQuery {
    pub raw_query: String,
    pub query_text: String,
    pub media_types: Vec<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub min_confidence: Option<f32>,
    pub root_hints: Vec<String>,
    pub parser_confidence: f32,
}

impl ParsedQuery {
    pub fn passthrough(raw: &str) -> Self {
        Self {
            raw_query: raw.to_string(),
            query_text: raw.to_string(),
            media_types: Vec::new(),
            date_from: None,
            date_to: None,
            min_confidence: None,
            root_hints: Vec::new(),
            parser_confidence: 0.2,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root_id: i64,
    pub root_path: String,
    pub scanned: u64,
    pub added: u64,
    pub modified: u64,
    pub moved: u64,
    pub unchanged: u64,
    pub deleted: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanJobStatus {
    pub id: i64,
    pub root_id: i64,
    pub root_path: String,
    pub status: String,
    pub scan_marker: i64,
    pub total_files: u64,
    pub processed_files: u64,
    pub progress_pct: f32,
    pub added: u64,
    pub modified: u64,
    pub moved: u64,
    pub unchanged: u64,
    pub deleted: u64,
    pub cursor_rel_path: Option<String>,
    pub error_text: Option<String>,
    pub updated_at: i64,
    pub started_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ScanJobState {
    pub root_id: i64,
    pub root_path: String,
    pub scan_marker: i64,
    pub processed_files: u64,
    pub added: u64,
    pub modified: u64,
    pub moved: u64,
    pub unchanged: u64,
    pub cursor_rel_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub current_model: Option<String>,
    pub loaded_models: Vec<String>,
    pub vram_used_mib: Option<u64>,
    pub vram_total_mib: Option<u64>,
    pub ollama_available: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupDownloadStatus {
    pub status: String,
    pub model: Option<String>,
    pub progress_pct: f32,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub is_ready: bool,
    pub ollama_available: bool,
    pub required_models: Vec<String>,
    pub missing_models: Vec<String>,
    pub instructions: Vec<String>,
    pub download: SetupDownloadStatus,
    pub python_available: bool,
    pub python_version: Option<String>,
    pub surya_venv_ok: bool,
}

#[derive(Debug, Clone)]
pub struct FileRecordUpsert {
    pub root_id: i64,
    pub rel_path: String,
    pub abs_path: String,
    pub filename: String,
    pub media_type: String,
    pub description: String,
    pub extracted_text: String,
    pub canonical_mentions: String,
    pub confidence: f32,
    pub lang_hint: String,
    pub mtime_ns: i64,
    pub size_bytes: i64,
    pub fingerprint: String,
    pub scan_marker: i64,
}

#[derive(Debug, Clone)]
pub struct ExistingFile {
    pub id: i64,
    pub rel_path: String,
    pub fingerprint: String,
    pub mtime_ns: i64,
    pub size_bytes: i64,
    #[allow(dead_code)] // Used for has_classification checks in future
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct ScanContext {
    pub db_path: std::path::PathBuf,
    pub thumbnails_dir: std::path::PathBuf,
    pub tmp_dir: std::path::PathBuf,
    pub surya_venv_dir: std::path::PathBuf,
    pub surya_script: std::path::PathBuf,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub media_type: String,
    pub description: String,
    pub extracted_text: String,
    pub canonical_mentions: String,
    pub confidence: f32,
    pub lang_hint: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub id: i64,
    pub root_path: String,
    pub root_name: String,
    pub created_at: i64,
    pub last_scan_at: Option<i64>,
    pub file_count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeResult {
    pub files_removed: u64,
    pub jobs_removed: u64,
    pub thumbs_cleaned: u64,
}

#[derive(Debug, Clone)]
pub enum HealthCheckOutcome {
    Healthy,
    RestoredFromBackup,
    Recreated,
}

/// Internal struct for filesystem cleanup after DB deletion — not serialized to frontend.
#[derive(Debug, Clone)]
pub struct DeletedFileInfo {
    pub abs_path: String,
    pub thumb_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFilesResult {
    pub deleted_count: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFileResult {
    pub file_id: i64,
    pub new_rel_path: String,
    pub new_abs_path: String,
    pub new_filename: String,
}
