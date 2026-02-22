#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub status: String,
    pub mode: String,
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
}
