//! Lightweight localhost HTTP server for video streaming.
//!
//! WebKit2GTK on Linux does not support video/audio playback through Tauri's
//! asset protocol (custom URI scheme).  This module spins up a minimal HTTP
//! server on `127.0.0.1` (random port) that serves video files with proper
//! `Range` header support so the `<video>` element can seek and play.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::OnceLock;

/// Port of the running server (set once, never changes).
static SERVER_PORT: OnceLock<u16> = OnceLock::new();

/// Start the video server (idempotent – second call returns the same port).
pub fn ensure_running() -> u16 {
    *SERVER_PORT.get_or_init(|| {
        let listener = TcpListener::bind("127.0.0.1:0").expect("video server: bind failed");
        let port = listener
            .local_addr()
            .expect("video server: local_addr")
            .port();
        log::info!("Video streaming server listening on 127.0.0.1:{port}");

        std::thread::Builder::new()
            .name("video-server".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    // Handle each connection in a short-lived thread so one
                    // slow client cannot block others.
                    std::thread::Builder::new()
                        .name("video-conn".into())
                        .spawn(move || {
                            if let Err(e) = handle_connection(stream) {
                                log::debug!("video server connection error: {e}");
                            }
                        })
                        .ok();
                }
            })
            .expect("video server: spawn thread");

        port
    })
}

/// Build the streaming URL for a given absolute file path.
pub fn stream_url(abs_path: &str) -> String {
    let port = ensure_running();
    let encoded = urlencod(abs_path);
    format!("http://127.0.0.1:{port}/video?path={encoded}")
}

// ── HTTP handling ──────────────────────────────────────────────────────

fn handle_connection(mut stream: std::net::TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;

    let mut reader = BufReader::new(stream.try_clone()?);

    // Read request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Read headers
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, val)) = trimmed.split_once(':') {
            headers.insert(key.trim().to_ascii_lowercase(), val.trim().to_string());
        }
    }

    // Parse: GET /video?path=<encoded> HTTP/1.x
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "GET" {
        return send_error(&mut stream, 405, "Method Not Allowed");
    }

    let (path_part, query) = parts[1].split_once('?').unwrap_or((parts[1], ""));
    if path_part != "/video" {
        return send_error(&mut stream, 404, "Not Found");
    }

    let file_path = parse_query_param(query, "path").unwrap_or_default();
    if file_path.is_empty() {
        return send_error(&mut stream, 400, "Missing path parameter");
    }

    let file_path = urldecod(file_path);
    let path = Path::new(&file_path);

    if !path.is_file() {
        return send_error(&mut stream, 404, "File Not Found");
    }

    // Validate it's a video file
    if !crate::video::is_video_file(path) {
        return send_error(&mut stream, 403, "Not a video file");
    }

    let mime = mime_for_ext(path);
    let file_size = std::fs::metadata(path)?.len();

    // Parse Range header
    let range = headers.get("range").and_then(|r| parse_range(r, file_size));

    let mut file = std::fs::File::open(path)?;

    match range {
        Some((start, end)) => {
            let length = end - start + 1;
            file.seek(SeekFrom::Start(start))?;

            let header = format!(
                "HTTP/1.1 206 Partial Content\r\n\
                 Content-Type: {mime}\r\n\
                 Content-Length: {length}\r\n\
                 Content-Range: bytes {start}-{end}/{file_size}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 Connection: close\r\n\
                 \r\n"
            );
            stream.write_all(header.as_bytes())?;
            copy_n(&mut file, &mut stream, length)?;
        }
        None => {
            let header = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {mime}\r\n\
                 Content-Length: {file_size}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 Connection: close\r\n\
                 \r\n"
            );
            stream.write_all(header.as_bytes())?;
            copy_n(&mut file, &mut stream, file_size)?;
        }
    }

    stream.flush()?;
    Ok(())
}

/// Copy exactly `n` bytes from `reader` to `writer` (chunked).
fn copy_n(
    reader: &mut impl Read,
    writer: &mut impl Write,
    mut remaining: u64,
) -> std::io::Result<()> {
    let mut buf = [0u8; 64 * 1024];
    while remaining > 0 {
        let to_read = (remaining as usize).min(buf.len());
        let n = reader.read(&mut buf[..to_read])?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        remaining -= n as u64;
    }
    Ok(())
}

fn send_error(stream: &mut std::net::TcpStream, code: u16, reason: &str) -> std::io::Result<()> {
    let body = format!("{code} {reason}");
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    stream.write_all(resp.as_bytes())?;
    stream.flush()
}

/// Parse `Range: bytes=START-END` header. Only single ranges supported.
fn parse_range(header: &str, file_size: u64) -> Option<(u64, u64)> {
    let s = header.strip_prefix("bytes=")?;
    let (start_str, end_str) = s.split_once('-')?;

    if start_str.is_empty() {
        // Suffix range: bytes=-500 (last 500 bytes)
        let suffix: u64 = end_str.parse().ok()?;
        let start = file_size.saturating_sub(suffix);
        Some((start, file_size - 1))
    } else {
        let start: u64 = start_str.parse().ok()?;
        let end = if end_str.is_empty() {
            file_size - 1
        } else {
            end_str.parse::<u64>().ok()?.min(file_size - 1)
        };
        if start > end || start >= file_size {
            return None;
        }
        Some((start, end))
    }
}

fn parse_query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            Some(v)
        } else {
            None
        }
    })
}

fn mime_for_ext(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        Some("wmv") => "video/x-ms-wmv",
        Some("flv") => "video/x-flv",
        Some("ts") => "video/mp2t",
        Some("mpg" | "mpeg") => "video/mpeg",
        _ => "application/octet-stream",
    }
}

/// Minimal percent-encoding for path characters.
fn urlencod(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Decode percent-encoded string.
fn urldecod(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(val);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_full() {
        assert_eq!(parse_range("bytes=0-999", 1000), Some((0, 999)));
    }

    #[test]
    fn parse_range_open_end() {
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
    }

    #[test]
    fn parse_range_suffix() {
        assert_eq!(parse_range("bytes=-200", 1000), Some((800, 999)));
    }

    #[test]
    fn parse_range_out_of_bounds() {
        assert_eq!(parse_range("bytes=1000-", 1000), None);
    }

    #[test]
    fn parse_range_clamps_end() {
        assert_eq!(parse_range("bytes=0-9999", 1000), Some((0, 999)));
    }

    #[test]
    fn url_encode_decode_roundtrip() {
        let path = "/home/user/My Videos/movie (2024).mp4";
        let encoded = urlencod(path);
        let decoded = urldecod(&encoded);
        assert_eq!(decoded, path);
    }

    #[test]
    fn mime_mp4() {
        assert_eq!(mime_for_ext(Path::new("test.mp4")), "video/mp4");
    }

    #[test]
    fn mime_mkv() {
        assert_eq!(mime_for_ext(Path::new("test.mkv")), "video/x-matroska");
    }

    #[test]
    fn mime_webm() {
        assert_eq!(mime_for_ext(Path::new("test.webm")), "video/webm");
    }

    #[test]
    fn query_param_extraction() {
        assert_eq!(parse_query_param("path=/foo&bar=baz", "path"), Some("/foo"));
        assert_eq!(parse_query_param("path=/foo&bar=baz", "bar"), Some("baz"));
        assert_eq!(parse_query_param("path=/foo", "missing"), None);
    }
}
