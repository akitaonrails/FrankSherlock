use regex::Regex;

use crate::models::ParsedQuery;

pub fn parse_query(raw_query: &str) -> ParsedQuery {
    let raw = raw_query.trim();
    if raw.is_empty() {
        return ParsedQuery::passthrough("");
    }

    let lower = raw.to_lowercase();
    let mut media_types = Vec::new();
    let mut root_hints = Vec::new();
    let mut min_confidence = None;
    let mut date_from = None;
    let mut date_to = None;
    let mut score = 0.25_f32;

    if lower.contains("receipt")
        || lower.contains("invoice")
        || lower.contains("comprovante")
        || lower.contains("bank")
        || lower.contains("statement")
    {
        media_types.push("document".to_string());
        score += 0.2;
    }
    if lower.contains("anime")
        || lower.contains("manga")
        || lower.contains("girl character")
        || lower.contains("waifu")
    {
        media_types.push("anime".to_string());
        score += 0.2;
    }
    if lower.contains("screenshot") {
        media_types.push("screenshot".to_string());
        score += 0.1;
    }
    if lower.contains("photo") {
        media_types.push("photo".to_string());
        score += 0.1;
    }
    dedup(&mut media_types);

    if let Some(conf) = parse_min_confidence(raw) {
        min_confidence = Some(conf);
        score += 0.15;
    }

    let root_re = Regex::new(r"(?i)\bin\s+([A-Za-z0-9._/\-]+)").expect("valid regex");
    for cap in root_re.captures_iter(raw) {
        if let Some(v) = cap.get(1) {
            root_hints.push(v.as_str().to_string());
        }
    }
    if !root_hints.is_empty() {
        score += 0.1;
    }

    if let Some((start, end)) = parse_date_range(raw) {
        date_from = Some(start);
        date_to = end;
        score += 0.2;
    }

    ParsedQuery {
        raw_query: raw.to_string(),
        query_text: raw.to_string(),
        media_types,
        date_from,
        date_to,
        min_confidence,
        root_hints,
        parser_confidence: score.clamp(0.0, 1.0),
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
        let parsed = parse_query("receipts between 2023 and 2024 in Dropbox");
        assert_eq!(parsed.date_from.as_deref(), Some("2023-01-01"));
        assert_eq!(parsed.date_to.as_deref(), Some("2024-12-31"));
        assert_eq!(parsed.root_hints, vec!["Dropbox".to_string()]);
        assert!(parsed.media_types.contains(&"document".to_string()));
    }

    #[test]
    fn passthrough_on_empty() {
        let parsed = parse_query(" ");
        assert_eq!(parsed.query_text, "");
        assert!(parsed.media_types.is_empty());
    }
}
