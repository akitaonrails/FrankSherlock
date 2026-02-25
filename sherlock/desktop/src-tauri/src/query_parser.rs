use regex::Regex;

use crate::models::ParsedQuery;

/// (regex_pattern, media_type, score_bump) — multi-word patterns first so they match before singles.
const MEDIA_TRIGGERS: &[(&str, &str, f32)] = &[
    // Multi-word (must come before single-word components)
    (r"girl\s+character", "anime", 0.2),
    (r"tv\s+shows?", "video", 0.2),
    // Single-word
    (r"receipts?", "document", 0.2),
    (r"invoices?", "document", 0.2),
    (r"comprovantes?", "document", 0.2),
    (r"bank", "document", 0.2),
    (r"statements?", "document", 0.2),
    (r"anime", "anime", 0.2),
    (r"manga", "anime", 0.2),
    (r"waifus?", "anime", 0.2),
    (r"screenshots?", "screenshot", 0.1),
    (r"videos?", "video", 0.2),
    (r"movies?", "video", 0.2),
    (r"episodes?", "video", 0.2),
    (r"films?", "video", 0.2),
    (r"photos?", "photo", 0.1),
];

pub fn parse_query(raw_query: &str) -> ParsedQuery {
    let raw = raw_query.trim();
    if raw.is_empty() {
        return ParsedQuery::passthrough("");
    }

    // Extract album:name or album:"name with spaces" prefix
    let mut album_name: Option<String> = None;
    let album_re = Regex::new(r#"(?i)\balbum:(?:"([^"]+)"|(\S+))"#).expect("valid regex");
    let working_query = if let Some(cap) = album_re.captures(raw) {
        album_name = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str().to_string());
        album_re.replace(raw, "").trim().to_string()
    } else {
        raw.to_string()
    };

    // Extract subdir:path or subdir:"path with spaces" prefix
    let mut subdir: Option<String> = None;
    let subdir_re = Regex::new(r#"(?i)\bsubdir:(?:"([^"]+)"|(\S+))"#).expect("valid regex");
    let working_query = if let Some(cap) = subdir_re.captures(&working_query) {
        subdir = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str().to_string());
        subdir_re.replace(&working_query, "").trim().to_string()
    } else {
        working_query
    };

    let mut media_types = Vec::new();
    let mut min_confidence = None;
    let mut date_from = None;
    let mut date_to = None;
    let mut score = 0.25_f32;

    // Detect media-type triggers and collect patterns to strip
    let mut matched_patterns: Vec<(Regex, usize)> = Vec::new();
    for &(pattern, media_type, bump) in MEDIA_TRIGGERS {
        let re = Regex::new(&format!(r"(?i)\b{}\b", pattern)).expect("valid trigger regex");
        if re.is_match(&working_query) {
            if !media_types.contains(&media_type.to_string()) {
                media_types.push(media_type.to_string());
                score += bump;
            }
            // Track pattern length (approximate) for longest-first stripping
            matched_patterns.push((re, pattern.len()));
        }
    }
    dedup(&mut media_types);

    if let Some(conf) = parse_min_confidence(&working_query) {
        min_confidence = Some(conf);
        score += 0.15;
    }

    if let Some((start, end)) = parse_date_range(&working_query) {
        date_from = Some(start);
        date_to = end;
        score += 0.2;
    }

    if album_name.is_some() {
        score += 0.3;
    }
    if subdir.is_some() {
        score += 0.2;
    }

    // Strip matched media keywords from query text, longest patterns first
    let mut query_text = working_query.clone();
    matched_patterns.sort_by(|a, b| b.1.cmp(&a.1));
    for (re, _) in &matched_patterns {
        query_text = re.replace_all(&query_text, " ").to_string();
    }
    // Collapse whitespace and trim
    let ws_re = Regex::new(r"\s+").expect("valid regex");
    let query_text = ws_re.replace_all(query_text.trim(), " ").to_string();

    ParsedQuery {
        raw_query: raw.to_string(),
        query_text,
        media_types,
        date_from,
        date_to,
        min_confidence,
        parser_confidence: score.clamp(0.0, 1.0),
        album_name,
        subdir,
    }
}

fn parse_min_confidence(raw: &str) -> Option<f32> {
    let re = Regex::new(
        r"(?i)(?:confidence|min(?:imum)?\s+confidence)\s*(?:>=|>|=)?\s*(0(?:\.\d+)?|1(?:\.0+)?)",
    )
    .expect("valid regex");
    let value = re
        .captures(raw)
        .and_then(|caps| caps.get(1))
        .and_then(|v| v.as_str().parse::<f32>().ok())?;
    Some(value.clamp(0.0, 1.0))
}

fn parse_date_range(raw: &str) -> Option<(String, Option<String>)> {
    let between_re = Regex::new(r"(?i)\bbetween\s+(\d{4})\s+and\s+(\d{4})\b").expect("valid regex");
    if let Some(cap) = between_re.captures(raw) {
        let from_year = cap.get(1)?.as_str();
        let to_year = cap.get(2)?.as_str();
        return Some((
            format!("{from_year}-01-01"),
            Some(format!("{to_year}-12-31")),
        ));
    }

    let from_re = Regex::new(r"(?i)\bfrom\s+(\d{4})\b").expect("valid regex");
    if let Some(cap) = from_re.captures(raw) {
        let year = cap.get(1)?.as_str();
        return Some((format!("{year}-01-01"), None));
    }

    let iso_re = Regex::new(r"\b(\d{4}-\d{2}-\d{2})\b").expect("valid regex");
    let mut matches = iso_re.captures_iter(raw);
    if let Some(first) = matches.next() {
        if let Some(first_date) = first.get(1) {
            if let Some(second) = matches.next() {
                if let Some(second_date) = second.get(1) {
                    return Some((
                        first_date.as_str().to_string(),
                        Some(second_date.as_str().to_string()),
                    ));
                }
            }
            return Some((first_date.as_str().to_string(), None));
        }
    }

    None
}

fn dedup(values: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    values.retain(|v| seen.insert(v.to_lowercase()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_and_confidence() {
        let parsed = parse_query("find anime girl character confidence >= 0.8");
        assert!(parsed.media_types.contains(&"anime".to_string()));
        assert_eq!(parsed.min_confidence, Some(0.8));
        assert!(parsed.parser_confidence > 0.5);
    }

    #[test]
    fn parses_between_year_range() {
        let parsed = parse_query("receipts between 2023 and 2024");
        assert_eq!(parsed.date_from.as_deref(), Some("2023-01-01"));
        assert_eq!(parsed.date_to.as_deref(), Some("2024-12-31"));
        assert!(parsed.media_types.contains(&"document".to_string()));
    }

    #[test]
    fn passthrough_on_empty() {
        let parsed = parse_query(" ");
        assert_eq!(parsed.query_text, "");
        assert!(parsed.media_types.is_empty());
    }

    #[test]
    fn parses_album_prefix_simple() {
        let parsed = parse_query("album:vacation beach");
        assert_eq!(parsed.album_name.as_deref(), Some("vacation"));
        assert_eq!(parsed.query_text, "beach");
    }

    #[test]
    fn parses_album_prefix_quoted() {
        let parsed = parse_query("album:\"my trip\" sunset");
        assert_eq!(parsed.album_name.as_deref(), Some("my trip"));
        assert_eq!(parsed.query_text, "sunset");
    }

    #[test]
    fn album_only_query() {
        let parsed = parse_query("album:favorites");
        assert_eq!(parsed.album_name.as_deref(), Some("favorites"));
        assert_eq!(parsed.query_text, "");
    }

    #[test]
    fn no_album_prefix() {
        let parsed = parse_query("beach sunset");
        assert!(parsed.album_name.is_none());
        assert_eq!(parsed.query_text, "beach sunset");
    }

    #[test]
    fn parses_free_text_only() {
        let parsed = parse_query("ranma");
        assert_eq!(parsed.query_text, "ranma");
        assert!(parsed.media_types.is_empty());
        assert!(parsed.date_from.is_none());
        assert!(parsed.min_confidence.is_none());
    }

    #[test]
    fn parses_photo_media_type() {
        let parsed = parse_query("photo beach");
        assert!(parsed.media_types.contains(&"photo".to_string()));
        assert_eq!(parsed.query_text, "beach");
    }

    #[test]
    fn parses_screenshot_media_type() {
        let parsed = parse_query("screenshot");
        assert!(parsed.media_types.contains(&"screenshot".to_string()));
    }

    #[test]
    fn parses_receipt_as_document() {
        let parsed = parse_query("receipt santander");
        assert!(parsed.media_types.contains(&"document".to_string()));
    }

    #[test]
    fn parses_from_year() {
        let parsed = parse_query("from 2022");
        assert_eq!(parsed.date_from.as_deref(), Some("2022-01-01"));
        assert!(parsed.date_to.is_none());
    }

    #[test]
    fn parses_single_iso_date() {
        let parsed = parse_query("2023-06-15");
        assert_eq!(parsed.date_from.as_deref(), Some("2023-06-15"));
        assert!(parsed.date_to.is_none());
    }

    #[test]
    fn parses_iso_date_range() {
        let parsed = parse_query("2023-01-01 2023-12-31");
        assert_eq!(parsed.date_from.as_deref(), Some("2023-01-01"));
        assert_eq!(parsed.date_to.as_deref(), Some("2023-12-31"));
    }

    #[test]
    fn parses_min_confidence_syntax() {
        let parsed = parse_query("min confidence > 0.7");
        assert_eq!(parsed.min_confidence, Some(0.7));
    }

    #[test]
    fn parses_combined_anime_date_range() {
        let parsed = parse_query("anime between 2023 and 2024");
        assert!(parsed.media_types.contains(&"anime".to_string()));
        assert_eq!(parsed.date_from.as_deref(), Some("2023-01-01"));
        assert_eq!(parsed.date_to.as_deref(), Some("2024-12-31"));
    }

    #[test]
    fn parses_combined_receipts_confidence() {
        let parsed = parse_query("receipts confidence >= 0.9");
        assert!(parsed.media_types.contains(&"document".to_string()));
        assert_eq!(parsed.min_confidence, Some(0.9));
    }

    #[test]
    fn parses_combined_photos_from_year() {
        let parsed = parse_query("photos from 2022");
        assert!(parsed.media_types.contains(&"photo".to_string()));
        assert_eq!(parsed.date_from.as_deref(), Some("2022-01-01"));
    }

    #[test]
    fn parses_video_media_type() {
        let parsed = parse_query("video sunset");
        assert!(parsed.media_types.contains(&"video".to_string()));
    }

    #[test]
    fn parses_movie_as_video() {
        let parsed = parse_query("movie action");
        assert!(parsed.media_types.contains(&"video".to_string()));
    }

    #[test]
    fn parses_episode_as_video() {
        let parsed = parse_query("episode season 1");
        assert!(parsed.media_types.contains(&"video".to_string()));
    }

    #[test]
    fn parses_film_as_video() {
        let parsed = parse_query("film noir");
        assert!(parsed.media_types.contains(&"video".to_string()));
    }

    #[test]
    fn parses_tv_show_as_video() {
        let parsed = parse_query("tv show comedy");
        assert!(parsed.media_types.contains(&"video".to_string()));
    }

    // -----------------------------------------------------------------------
    // subdir: prefix tests
    // -----------------------------------------------------------------------

    #[test]
    fn parses_subdir_simple() {
        let parsed = parse_query("subdir:Screenshots beach");
        assert_eq!(parsed.subdir.as_deref(), Some("Screenshots"));
        assert_eq!(parsed.query_text, "beach");
    }

    #[test]
    fn parses_subdir_quoted() {
        let parsed = parse_query("subdir:\"Photos/2024\" sunset");
        assert_eq!(parsed.subdir.as_deref(), Some("Photos/2024"));
        assert_eq!(parsed.query_text, "sunset");
    }

    #[test]
    fn subdir_only_query() {
        let parsed = parse_query("subdir:Downloads");
        assert_eq!(parsed.subdir.as_deref(), Some("Downloads"));
        assert_eq!(parsed.query_text, "");
    }

    #[test]
    fn no_subdir_prefix() {
        let parsed = parse_query("beach sunset");
        assert!(parsed.subdir.is_none());
        assert_eq!(parsed.query_text, "beach sunset");
    }

    // -----------------------------------------------------------------------
    // Media keyword stripping tests
    // -----------------------------------------------------------------------

    #[test]
    fn strips_media_keywords_from_query_text() {
        let parsed = parse_query("photo beach sunset");
        assert!(parsed.media_types.contains(&"photo".to_string()));
        assert_eq!(parsed.query_text, "beach sunset");
    }

    #[test]
    fn strips_plural_media_keywords() {
        let parsed = parse_query("photos from 2022");
        assert!(parsed.media_types.contains(&"photo".to_string()));
        assert_eq!(parsed.query_text, "from 2022");
    }

    #[test]
    fn strips_multi_word_trigger() {
        let parsed = parse_query("tv show comedy");
        assert!(parsed.media_types.contains(&"video".to_string()));
        assert_eq!(parsed.query_text, "comedy");
    }

    #[test]
    fn media_only_query_produces_empty_text() {
        let parsed = parse_query("screenshot");
        assert!(parsed.media_types.contains(&"screenshot".to_string()));
        assert_eq!(parsed.query_text, "");
    }
}
