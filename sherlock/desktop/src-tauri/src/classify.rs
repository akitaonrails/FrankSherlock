use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use regex::Regex;
use serde_json::Value;

use crate::models::ClassificationResult;

// ---------------------------------------------------------------------------
// Prompts (identical to Python prototype)
// ---------------------------------------------------------------------------

const PRIMARY_PROMPT: &str = concat!(
    "Analyze this image and respond ONLY with valid JSON. Schema: ",
    r#"{"media_type":"screenshot|anime|manga|photo|document|artwork|other","#,
    r#""contains_text":true,"#,
    r#""is_anime_related":false,"#,
    r#""is_document_like":false,"#,
    r#""description":"short factual description","#,
    r#""series_candidates":["name"],"#,
    r#""character_candidates":["name"],"#,
    r#""confidence":0.0}"#,
    " Rules: ",
    "1) Use null-like empty arrays when unknown. ",
    "2) If image has visible text (UI, receipt, scan, subtitles), contains_text=true. ",
    "3) is_document_like=true for receipts/invoices/forms/scanned docs/screenshots of documents. ",
    "4) series_candidates and character_candidates must be unique and max 5 items each. ",
    "5) Keep description under 24 words. ",
    "6) Favor precision over guesswork.",
);

const PRIMARY_PROMPT_FALLBACK: &str = concat!(
    "Return ONLY valid compact JSON with schema: ",
    r#"{"media_type":"screenshot|anime|manga|photo|document|artwork|other","#,
    r#""contains_text":true,"#,
    r#""is_anime_related":false,"#,
    r#""is_document_like":false,"#,
    r#""description":"max 24 words","#,
    r#""series_candidates":["max 3 unique names"],"#,
    r#""character_candidates":["max 3 unique names"],"#,
    r#""confidence":0.0}"#,
    " Never exceed 3 items in candidates arrays. Never repeat entries. No markdown.",
);

const ANIME_PROMPT: &str = concat!(
    "This appears anime/manga-related. Return ONLY valid JSON with schema: ",
    r#"{"series":"name or null","#,
    r#""franchise":"name or null","#,
    r#""characters":[{"name":"full canonical name","series":"name or null","confidence":0.0}],"#,
    r#""canonical_mentions":["Name from Series"],"#,
    r#""scene_summary":"short","#,
    r#""confidence":0.0}"#,
    " Rules: prefer canonical full names when possible; if unknown set null/empty.",
);

const OCR_PROMPT: &str =
    "Extract ALL visible text exactly as seen. Return ONLY raw text. Preserve line breaks.";

const RECEIPT_PROMPT: &str = concat!(
    "Given OCR text from a potential receipt/bank document, extract structured fields. ",
    "Return ONLY valid JSON schema: ",
    r#"{"document_kind":"receipt|invoice|bank_transfer|statement|other","#,
    r#""issuer":"string or null","#,
    r#""counterparty":"string or null","#,
    r#""date":"YYYY-MM-DD or null","#,
    r#""time":"HH:MM:SS or null","#,
    r#""amount":"string or null","#,
    r#""currency":"BRL|USD|EUR|other|null","#,
    r#""transaction_id":"string or null","#,
    r#""reference_numbers":["string"],"#,
    r#""important_fields":[{"key":"string","value":"string"}],"#,
    r#""confidence":0.0}"#,
    " OCR text:\n",
);

const VALID_MEDIA_TYPES: &[&str] = &[
    "screenshot",
    "anime",
    "manga",
    "photo",
    "document",
    "artwork",
    "other",
];

// ---------------------------------------------------------------------------
// Ollama HTTP client
// ---------------------------------------------------------------------------

struct OllamaResponse {
    ok: bool,
    raw: String,
    #[allow(dead_code)]
    total_duration_s: f64,
}

fn ollama_generate(
    model: &str,
    prompt: &str,
    image_path: Option<&Path>,
    num_predict: u32,
    timeout_secs: u64,
    json_mode: bool,
) -> OllamaResponse {
    let mut payload = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "keep_alive": "5m",
        "options": {
            "temperature": 0.1,
            "num_predict": num_predict,
        }
    });

    if json_mode {
        payload["format"] = serde_json::json!("json");
    }

    if let Some(img_path) = image_path {
        match std::fs::read(img_path) {
            Ok(bytes) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                payload["images"] = serde_json::json!([b64]);
            }
            Err(e) => {
                return OllamaResponse {
                    ok: false,
                    raw: format!("image_read_error: {e}"),
                    total_duration_s: 0.0,
                };
            }
        }
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(timeout_secs))
        .timeout_write(Duration::from_secs(30))
        .build();

    let result = agent
        .post("http://localhost:11434/api/generate")
        .send_json(&payload);

    match result {
        Ok(resp) => {
            let body: String = match resp.into_string() {
                Ok(s) => s,
                Err(e) => {
                    return OllamaResponse {
                        ok: false,
                        raw: format!("read_error: {e}"),
                        total_duration_s: 0.0,
                    };
                }
            };
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let raw = parsed
                .get("response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let dur = parsed
                .get("total_duration")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
                / 1_000_000_000.0;
            OllamaResponse {
                ok: true,
                raw,
                total_duration_s: dur,
            }
        }
        Err(e) => OllamaResponse {
            ok: false,
            raw: format!("http_error: {e}"),
            total_duration_s: 0.0,
        },
    }
}

// ---------------------------------------------------------------------------
// JSON parsing fallbacks
// ---------------------------------------------------------------------------

/// Extract the first balanced JSON object from free-form text.
pub fn extract_first_json_object(text: &str) -> Option<Value> {
    if text.is_empty() {
        return None;
    }
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            if v.is_object() {
                return Some(v);
            }
        }
    }
    for (start, _) in text.char_indices().filter(|(_, c)| *c == '{') {
        let mut depth = 0i32;
        for (i, ch) in text[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let candidate = &text[start..start + i + 1];
                        if let Ok(v) = serde_json::from_str::<Value>(candidate) {
                            if v.is_object() {
                                return Some(v);
                            }
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn parse_json_response(raw: &str) -> Option<Value> {
    if raw.is_empty() {
        return None;
    }
    extract_first_json_object(raw)
}

/// Regex-based salvage when all JSON attempts fail.
pub fn salvage_primary_from_raw(raw: &str) -> Option<Value> {
    if raw.is_empty() {
        return None;
    }
    let mt_re = Regex::new(r#"(?i)"media_type"\s*:\s*"([^"]+)""#).ok()?;
    let mt_match = mt_re.captures(raw)?;
    let media_type = mt_match.get(1)?.as_str().trim().to_lowercase();
    let media_type = if VALID_MEDIA_TYPES.contains(&media_type.as_str()) {
        media_type
    } else {
        "other".to_string()
    };

    fn extract_bool(raw: &str, key: &str, default: bool) -> bool {
        let pattern = format!(r#"(?i)"{key}"\s*:\s*(true|false)"#);
        Regex::new(&pattern)
            .ok()
            .and_then(|re| re.captures(raw))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().eq_ignore_ascii_case("true"))
            .unwrap_or(default)
    }

    let contains_text = extract_bool(raw, "contains_text", false);
    let is_anime = extract_bool(
        raw,
        "is_anime_related",
        matches!(media_type.as_str(), "anime" | "manga" | "artwork"),
    );
    let is_doc = extract_bool(
        raw,
        "is_document_like",
        matches!(media_type.as_str(), "document" | "screenshot"),
    );

    let desc_re = Regex::new(r#"(?i)"description"\s*:\s*"([^"]*)""#).ok()?;
    let desc = desc_re
        .captures(raw)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();

    let conf_re = Regex::new(r#"(?i)"confidence"\s*:\s*([0-9]*\.?[0-9]+)"#).ok()?;
    let confidence: f64 = conf_re
        .captures(raw)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0.0);

    let series = extract_quoted_items(raw, "series_candidates", 5);
    let chars = extract_quoted_items(raw, "character_candidates", 5);

    Some(serde_json::json!({
        "media_type": media_type,
        "contains_text": contains_text,
        "is_anime_related": is_anime,
        "is_document_like": is_doc,
        "description": desc,
        "series_candidates": series,
        "character_candidates": chars,
        "confidence": confidence,
    }))
}

fn extract_quoted_items(raw: &str, key: &str, limit: usize) -> Vec<String> {
    let pattern = format!(r#"(?is)"{key}"\s*:\s*\[([^\]]*)"#);
    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let fragment = match re.captures(raw).and_then(|c| c.get(1)) {
        Some(m) => m.as_str(),
        None => return Vec::new(),
    };
    let item_re = Regex::new(r#""([^"]+)""#).unwrap();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for cap in item_re.captures_iter(fragment) {
        let val = cap[1].trim().to_string();
        if val.is_empty() {
            continue;
        }
        let key = val.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        out.push(val);
        if out.len() >= limit {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Normalize / clean helpers
// ---------------------------------------------------------------------------

pub fn normalize_list(value: &Value) -> Vec<String> {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| {
            !s.is_empty() && !matches!(s.to_lowercase().as_str(), "null" | "none" | "unknown")
        })
        .collect()
}

pub fn clean_nullable_str(value: &Value) -> Option<String> {
    let s = value.as_str()?.trim();
    if s.is_empty()
        || matches!(
            s.to_lowercase().as_str(),
            "null" | "none" | "unknown" | "n/a"
        )
    {
        return None;
    }
    Some(s.to_string())
}

// ---------------------------------------------------------------------------
// Enrichment routing
// ---------------------------------------------------------------------------

pub fn should_run_anime_enrichment(primary: &Value) -> bool {
    let media_type = primary
        .get("media_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_anime = primary
        .get("is_anime_related")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    matches!(media_type, "anime" | "manga" | "artwork") || is_anime
}

pub fn should_run_document_enrichment(primary: &Value) -> bool {
    let media_type = primary
        .get("media_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let contains_text = primary
        .get("contains_text")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_doc = primary
        .get("is_document_like")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    matches!(media_type, "document" | "screenshot") || contains_text || is_doc
}

// ---------------------------------------------------------------------------
// Receipt regex extraction
// ---------------------------------------------------------------------------

pub fn extract_receipt_regex(text: &str) -> Value {
    let date_patterns = [
        r"\b(\d{4}-\d{2}-\d{2})\b",
        r"\b(\d{2}/\d{2}/\d{4})\b",
        r"\b(\d{2}-\d{2}-\d{4})\b",
    ];
    let amount_patterns = [
        r"\b(?:R\$|\$|EUR|USD)\s?[0-9][0-9.,]*\b",
        r"\b[0-9]{1,3}(?:\.[0-9]{3})*,[0-9]{2}\b",
        r"\b[0-9]+(?:\.[0-9]{2})\b",
    ];
    let transaction_patterns = [
        r"(?i)\b(?:ID|Tx|Transaction|Protocolo|Comprovante|Reference)[:\s#-]*([A-Za-z0-9\-_/]{6,})\b",
    ];

    fn collect_matches(text: &str, patterns: &[&'static str]) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for pat in patterns {
            if let Ok(re) = Regex::new(pat) {
                for cap in re.captures_iter(text) {
                    let val = cap
                        .get(1)
                        .unwrap_or_else(|| cap.get(0).unwrap())
                        .as_str()
                        .to_string();
                    if !seen.contains(&val) {
                        seen.insert(val.clone());
                        out.push(val);
                    }
                }
            }
        }
        out
    }

    let dates: Vec<String> = collect_matches(text, &date_patterns)
        .into_iter()
        .take(6)
        .collect();
    let amounts: Vec<String> = collect_matches(text, &amount_patterns)
        .into_iter()
        .take(10)
        .collect();
    let refs: Vec<String> = collect_matches(text, &transaction_patterns)
        .into_iter()
        .take(10)
        .collect();

    let currency = if text.contains("R$") {
        Some("BRL")
    } else if text.contains("USD") || text.contains('$') {
        Some("USD")
    } else if text.contains("EUR") {
        Some("EUR")
    } else {
        None
    };

    serde_json::json!({
        "dates": dates,
        "amount_candidates": amounts,
        "reference_numbers": refs,
        "currency_guess": currency,
    })
}

// ---------------------------------------------------------------------------
// GIF first-frame extraction
// ---------------------------------------------------------------------------

fn first_frame_if_gif(image_path: &Path, tmp_dir: &Path) -> PathBuf {
    if image_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .as_deref()
        != Some("gif")
    {
        return image_path.to_path_buf();
    }
    let stem = image_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "frame".to_string());
    let out = tmp_dir.join(format!("{stem}_frame1.png"));
    if out.exists() {
        return out;
    }
    let _ = std::fs::create_dir_all(tmp_dir);
    match image::open(image_path) {
        Ok(img) => {
            if let Err(e) = img.to_rgb8().save(&out) {
                log::warn!("Failed to save GIF first frame: {e}");
                return image_path.to_path_buf();
            }
            out
        }
        Err(e) => {
            log::warn!("Failed to open GIF for first frame: {e}");
            image_path.to_path_buf()
        }
    }
}

// ---------------------------------------------------------------------------
// Stage 1: Primary classification
// ---------------------------------------------------------------------------

fn classify_primary(model: &str, image_path: &Path, tmp_dir: &Path) -> Value {
    let effective_path = first_frame_if_gif(image_path, tmp_dir);

    // Attempt 1: primary prompt with json_mode
    let resp1 = ollama_generate(model, PRIMARY_PROMPT, Some(&effective_path), 500, 180, true);
    if resp1.ok {
        if let Some(v) = parse_json_response(&resp1.raw) {
            if v.get("media_type").and_then(|v| v.as_str()).is_some() {
                return v;
            }
        }
    }

    // Attempt 2: primary prompt without json_mode
    let prompt2 = format!("{PRIMARY_PROMPT} Return a single JSON object only.");
    let resp2 = ollama_generate(model, &prompt2, Some(&effective_path), 500, 180, false);
    if resp2.ok {
        if let Some(v) = parse_json_response(&resp2.raw) {
            if v.get("media_type").and_then(|v| v.as_str()).is_some() {
                return v;
            }
        }
    }

    // Attempt 3: fallback prompt with json_mode
    let resp3 = ollama_generate(
        model,
        PRIMARY_PROMPT_FALLBACK,
        Some(&effective_path),
        260,
        180,
        true,
    );
    if resp3.ok {
        if let Some(v) = parse_json_response(&resp3.raw) {
            if v.get("media_type").and_then(|v| v.as_str()).is_some() {
                return v;
            }
        }
    }

    // Salvage from raw attempts
    let combined = format!("{}\n{}\n{}", resp1.raw, resp2.raw, resp3.raw);
    if let Some(v) = salvage_primary_from_raw(&combined) {
        return v;
    }

    // Safe default
    serde_json::json!({
        "media_type": "other",
        "contains_text": false,
        "is_anime_related": false,
        "is_document_like": false,
        "description": "",
        "series_candidates": [],
        "character_candidates": [],
        "confidence": 0.0,
    })
}

// ---------------------------------------------------------------------------
// Stage 2a: Anime enrichment
// ---------------------------------------------------------------------------

fn classify_anime_details(model: &str, image_path: &Path, tmp_dir: &Path) -> Option<Value> {
    let effective_path = first_frame_if_gif(image_path, tmp_dir);

    let resp = ollama_generate(model, ANIME_PROMPT, Some(&effective_path), 600, 180, true);
    if resp.ok {
        if let Some(v) = parse_json_response(&resp.raw) {
            let conf = v.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0);
            let has_data = v.get("series").and_then(clean_nullable_str).is_some()
                || v.get("characters")
                    .and_then(|a| a.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
            if has_data || conf >= 0.2 {
                return Some(v);
            }
        }
    }

    // Retry without json_mode
    let prompt2 = format!("{ANIME_PROMPT} Return a single JSON object only.");
    let resp2 = ollama_generate(model, &prompt2, Some(&effective_path), 600, 180, false);
    if resp2.ok {
        if let Some(v) = parse_json_response(&resp2.raw) {
            return Some(v);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Stage 2b: OCR
// ---------------------------------------------------------------------------

fn run_surya_ocr(image_path: &Path, surya_venv: &Path, surya_script: &Path) -> OcrResult {
    let python_bin = crate::platform::python::python_venv_binary(surya_venv);
    if !python_bin.exists() || !surya_script.exists() {
        return OcrResult {
            ok: false,
            engine: "surya".to_string(),
            text: String::new(),
            line_count: 0,
        };
    }

    let result = Command::new(&python_bin)
        .arg(surya_script)
        .arg(image_path)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(v) = serde_json::from_str::<Value>(&stdout) {
                let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
                let text = v
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let line_count = v.get("line_count").and_then(|n| n.as_u64()).unwrap_or(0);
                return OcrResult {
                    ok,
                    engine: "surya".to_string(),
                    text,
                    line_count,
                };
            }
            OcrResult {
                ok: false,
                engine: "surya".to_string(),
                text: String::new(),
                line_count: 0,
            }
        }
        _ => OcrResult {
            ok: false,
            engine: "surya".to_string(),
            text: String::new(),
            line_count: 0,
        },
    }
}

fn run_llm_ocr(model: &str, image_path: &Path, tmp_dir: &Path) -> OcrResult {
    let effective_path = first_frame_if_gif(image_path, tmp_dir);
    let resp = ollama_generate(model, OCR_PROMPT, Some(&effective_path), 2000, 240, false);
    if !resp.ok {
        return OcrResult {
            ok: false,
            engine: "vision_llm".to_string(),
            text: String::new(),
            line_count: 0,
        };
    }
    let text = resp.raw.trim().to_string();
    let line_count = text.lines().count() as u64;
    OcrResult {
        ok: true,
        engine: "vision_llm".to_string(),
        text,
        line_count,
    }
}

struct OcrResult {
    ok: bool,
    #[allow(dead_code)]
    engine: String,
    text: String,
    #[allow(dead_code)]
    line_count: u64,
}

fn run_ocr(
    model: &str,
    image_path: &Path,
    tmp_dir: &Path,
    surya_venv: &Path,
    surya_script: &Path,
) -> OcrResult {
    let surya = run_surya_ocr(image_path, surya_venv, surya_script);
    if surya.ok && !surya.text.is_empty() {
        return surya;
    }
    run_llm_ocr(model, image_path, tmp_dir)
}

// ---------------------------------------------------------------------------
// Stage 2c: Document field extraction
// ---------------------------------------------------------------------------

fn extract_document_fields(model: &str, ocr_text: &str) -> Value {
    let regex_fields = extract_receipt_regex(ocr_text);

    let prompt = format!("{RECEIPT_PROMPT}{ocr_text}");
    let resp = ollama_generate(model, &prompt, None, 600, 180, true);
    let llm_fields = if resp.ok {
        parse_json_response(&resp.raw)
    } else {
        None
    };

    let mut doc = serde_json::json!({});
    if let Some(ref llm) = llm_fields {
        for key in [
            "document_kind",
            "issuer",
            "counterparty",
            "date",
            "time",
            "amount",
            "currency",
            "transaction_id",
            "reference_numbers",
            "important_fields",
            "confidence",
        ] {
            if let Some(v) = llm.get(key) {
                doc[key] = v.clone();
            }
        }
    }

    // Fall back to regex for currency and reference_numbers
    if doc.get("currency").and_then(clean_nullable_str).is_none() {
        if let Some(c) = regex_fields.get("currency_guess").and_then(|v| v.as_str()) {
            doc["currency"] = serde_json::json!(c);
        }
    }
    if doc
        .get("reference_numbers")
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true)
    {
        doc["reference_numbers"] = regex_fields
            .get("reference_numbers")
            .cloned()
            .unwrap_or(serde_json::json!([]));
    }

    // Transaction ID cleanup
    let txid = doc
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string());
    let mut final_txid = txid.clone();
    if let Some(ref t) = txid {
        let lower = t.to_lowercase();
        if lower == "ted realizado com sucesso." || lower == "ted realizado com sucesso" {
            final_txid = None;
        }
    }
    if final_txid.is_none() {
        if let Ok(re) = Regex::new(r"\b[A-Z0-9]{12,}\b") {
            let all_matches: Vec<String> = re
                .find_iter(ocr_text)
                .map(|m| m.as_str().to_string())
                .collect();
            if !all_matches.is_empty() {
                let has_digit = Regex::new(r"[0-9]").unwrap();
                let has_alpha = Regex::new(r"[A-Z]").unwrap();
                let mixed: Vec<&String> = all_matches
                    .iter()
                    .filter(|m| has_digit.is_match(m) && has_alpha.is_match(m))
                    .collect();
                let alpha: Vec<&String> = all_matches
                    .iter()
                    .filter(|m| has_alpha.is_match(m))
                    .collect();
                final_txid = Some(
                    mixed
                        .first()
                        .copied()
                        .or_else(|| alpha.first().copied())
                        .unwrap_or(&all_matches[0])
                        .clone(),
                );
            }
        }
    }
    if let Some(t) = final_txid {
        doc["transaction_id"] = serde_json::json!(t);
    } else {
        doc["transaction_id"] = Value::Null;
    }

    doc
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn classify_image(
    image_path: &Path,
    model: &str,
    tmp_dir: &Path,
    surya_venv: &Path,
    surya_script: &Path,
) -> ClassificationResult {
    log::info!("Classifying: {}", image_path.display());
    let primary = classify_primary(model, image_path, tmp_dir);

    let media_type = primary
        .get("media_type")
        .and_then(|v| v.as_str())
        .unwrap_or("other")
        .to_string();
    let description = primary
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let confidence = primary
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;

    let mut full_description = description.clone();
    let mut extracted_text = String::new();
    let mut canonical_mentions = String::new();

    // Anime enrichment
    if should_run_anime_enrichment(&primary) {
        if let Some(anime) = classify_anime_details(model, image_path, tmp_dir) {
            if let Some(series) = anime.get("series").and_then(clean_nullable_str) {
                full_description = format!("{full_description} [Series: {series}]");
            }
            let mentions = normalize_list(anime.get("canonical_mentions").unwrap_or(&Value::Null));
            if !mentions.is_empty() {
                canonical_mentions = mentions.join(", ");
            }
            if let Some(chars) = anime.get("characters").and_then(|v| v.as_array()) {
                let char_names: Vec<String> = chars
                    .iter()
                    .filter_map(|c| {
                        c.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();
                if !char_names.is_empty() && canonical_mentions.is_empty() {
                    canonical_mentions = char_names.join(", ");
                }
            }
        }
    }

    // Also add series/character candidates from primary to canonical_mentions
    let series_cands = normalize_list(primary.get("series_candidates").unwrap_or(&Value::Null));
    let char_cands = normalize_list(primary.get("character_candidates").unwrap_or(&Value::Null));
    let mut all_mentions: Vec<String> = Vec::new();
    if !canonical_mentions.is_empty() {
        all_mentions.extend(canonical_mentions.split(", ").map(|s| s.to_string()));
    }
    for c in series_cands.iter().chain(char_cands.iter()) {
        let lower = c.to_lowercase();
        if !all_mentions.iter().any(|m| m.to_lowercase() == lower) {
            all_mentions.push(c.clone());
        }
    }
    canonical_mentions = all_mentions.join(", ");

    // Document enrichment
    if should_run_document_enrichment(&primary) {
        let ocr = run_ocr(model, image_path, tmp_dir, surya_venv, surya_script);
        if ocr.ok && !ocr.text.is_empty() {
            extracted_text = ocr.text.clone();

            let doc_fields = extract_document_fields(model, &ocr.text);
            if let Some(kind) = doc_fields.get("document_kind").and_then(clean_nullable_str) {
                full_description = format!("{full_description} [Doc: {kind}]");
            }
            if let Some(issuer) = doc_fields.get("issuer").and_then(clean_nullable_str) {
                full_description = format!("{full_description} [Issuer: {issuer}]");
            }
        }
    }

    let lang_hint = detect_lang_hint(&extracted_text);

    ClassificationResult {
        media_type,
        description: full_description,
        extracted_text,
        canonical_mentions,
        confidence,
        lang_hint,
    }
}

fn detect_lang_hint(text: &str) -> String {
    if text.is_empty() {
        return "unknown".to_string();
    }
    let lower = text.to_lowercase();
    // Simple heuristic based on common words
    let pt_words = [
        "comprovante",
        "transferência",
        "valor",
        "banco",
        "realizado",
        "pagamento",
    ];
    let ja_chars = text
        .chars()
        .any(|c| ('\u{3040}'..='\u{30FF}').contains(&c) || ('\u{4E00}'..='\u{9FFF}').contains(&c));

    if ja_chars {
        return "ja".to_string();
    }
    if pt_words.iter().any(|w| lower.contains(w)) {
        return "pt".to_string();
    }
    "en".to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_first_json_object_valid() {
        let input = r#"{"media_type":"anime","confidence":0.9}"#;
        let result = extract_first_json_object(input).unwrap();
        assert_eq!(result["media_type"], "anime");
    }

    #[test]
    fn extract_first_json_object_garbage_prefix() {
        let input = r#"Here is my answer: {"media_type":"photo","confidence":0.5} done"#;
        let result = extract_first_json_object(input).unwrap();
        assert_eq!(result["media_type"], "photo");
    }

    #[test]
    fn extract_first_json_object_nested_braces() {
        let input = r#"{"a":{"b":1},"c":2}"#;
        let result = extract_first_json_object(input).unwrap();
        assert_eq!(result["c"], 2);
    }

    #[test]
    fn extract_first_json_object_empty() {
        assert!(extract_first_json_object("").is_none());
    }

    #[test]
    fn extract_first_json_object_no_json() {
        assert!(extract_first_json_object("just some text").is_none());
    }

    #[test]
    fn salvage_primary_with_all_fields() {
        let raw = r#"blah "media_type": "anime", "contains_text": true, "is_anime_related": true, "is_document_like": false, "description": "A girl", "confidence": 0.85, "series_candidates": ["Ranma"], "character_candidates": ["Akane"] blah"#;
        let v = salvage_primary_from_raw(raw).unwrap();
        assert_eq!(v["media_type"], "anime");
        assert_eq!(v["contains_text"], true);
        assert_eq!(v["is_anime_related"], true);
        assert_eq!(v["description"], "A girl");
        assert!((v["confidence"].as_f64().unwrap() - 0.85).abs() < 0.01);
    }

    #[test]
    fn salvage_primary_missing_media_type() {
        let raw = r#""description": "something""#;
        assert!(salvage_primary_from_raw(raw).is_none());
    }

    #[test]
    fn salvage_primary_partial_json() {
        let raw = r#"{"media_type":"photo","confidence":0.3"#;
        let v = salvage_primary_from_raw(raw).unwrap();
        assert_eq!(v["media_type"], "photo");
    }

    #[test]
    fn normalize_list_filters_nulls() {
        let v = serde_json::json!(["Ranma", "null", "", "None", "Akane", "unknown"]);
        let result = normalize_list(&v);
        assert_eq!(result, vec!["Ranma", "Akane"]);
    }

    #[test]
    fn normalize_list_non_array() {
        let v = serde_json::json!("not an array");
        assert!(normalize_list(&v).is_empty());
    }

    #[test]
    fn clean_nullable_str_variants() {
        assert!(clean_nullable_str(&serde_json::json!("null")).is_none());
        assert!(clean_nullable_str(&serde_json::json!("none")).is_none());
        assert!(clean_nullable_str(&serde_json::json!("N/A")).is_none());
        assert!(clean_nullable_str(&serde_json::json!("")).is_none());
        assert_eq!(
            clean_nullable_str(&serde_json::json!("Ranma")),
            Some("Ranma".to_string())
        );
    }

    #[test]
    fn should_run_anime_for_anime_type() {
        let p = serde_json::json!({"media_type": "anime", "is_anime_related": false});
        assert!(should_run_anime_enrichment(&p));
    }

    #[test]
    fn should_run_anime_for_flag() {
        let p = serde_json::json!({"media_type": "photo", "is_anime_related": true});
        assert!(should_run_anime_enrichment(&p));
    }

    #[test]
    fn should_not_run_anime_for_photo() {
        let p = serde_json::json!({"media_type": "photo", "is_anime_related": false});
        assert!(!should_run_anime_enrichment(&p));
    }

    #[test]
    fn should_run_document_for_doc_type() {
        let p = serde_json::json!({"media_type": "document", "contains_text": false, "is_document_like": false});
        assert!(should_run_document_enrichment(&p));
    }

    #[test]
    fn should_run_document_for_contains_text() {
        let p = serde_json::json!({"media_type": "photo", "contains_text": true, "is_document_like": false});
        assert!(should_run_document_enrichment(&p));
    }

    #[test]
    fn should_not_run_document_for_plain_photo() {
        let p = serde_json::json!({"media_type": "photo", "contains_text": false, "is_document_like": false});
        assert!(!should_run_document_enrichment(&p));
    }

    #[test]
    fn extract_receipt_regex_dates_amounts() {
        let text = "Date: 2024-01-15\nAmount: R$ 1.234,56\nRef: Protocolo: ABC123DEF";
        let result = extract_receipt_regex(text);
        let dates = result["dates"].as_array().unwrap();
        assert!(dates.iter().any(|d| d == "2024-01-15"));
        let amounts = result["amount_candidates"].as_array().unwrap();
        assert!(!amounts.is_empty());
        assert_eq!(result["currency_guess"], "BRL");
    }

    #[test]
    fn extract_receipt_regex_no_currency() {
        let text = "some random text without money";
        let result = extract_receipt_regex(text);
        assert!(result["currency_guess"].is_null());
    }

    #[test]
    fn detect_lang_hint_portuguese() {
        assert_eq!(detect_lang_hint("comprovante de pagamento"), "pt");
    }

    #[test]
    fn detect_lang_hint_japanese() {
        assert_eq!(detect_lang_hint("日本語のテキスト"), "ja");
    }

    #[test]
    fn detect_lang_hint_empty() {
        assert_eq!(detect_lang_hint(""), "unknown");
    }
}
