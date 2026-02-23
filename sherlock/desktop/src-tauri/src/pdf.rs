use std::path::Path;

use pdfium_render::prelude::*;

use crate::error::{AppError, AppResult};

/// Load PDFium from the bundled library path.
fn load_pdfium(lib_path: &Path) -> AppResult<Pdfium> {
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
        lib_path.parent().unwrap_or(lib_path),
    ))
    .map_err(|e| AppError::Config(format!("Failed to load PDFium library: {e}")))?;
    Ok(Pdfium::new(bindings))
}

/// Extract all text from a PDF, concatenating pages with "\n---\n" separators.
/// Returns (full_text, page_count).
pub fn extract_text(pdf_path: &Path, pdfium_lib: &Path) -> AppResult<(String, u32)> {
    let pdfium = load_pdfium(pdfium_lib)?;
    let doc = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| AppError::Config(format!("Failed to open PDF {}: {e}", pdf_path.display())))?;

    let page_count = doc.pages().len() as u32;
    let mut texts = Vec::with_capacity(page_count as usize);

    for page in doc.pages().iter() {
        let text = page.text().map_or_else(|_| String::new(), |t| t.all());
        texts.push(text);
    }

    let full_text = texts.join("\n---\n");
    Ok((full_text, page_count))
}

/// Check if a PDF is "scanned" (image-only) by checking if avg text per page < 50 chars.
pub fn is_scanned_pdf(full_text: &str, page_count: u32) -> bool {
    if page_count == 0 {
        return true;
    }
    let text_len = full_text.replace("\n---\n", "").trim().len();
    let avg_chars_per_page = text_len as f64 / page_count as f64;
    avg_chars_per_page < 50.0
}

/// Render a single page to an in-memory RGBA image buffer at a given DPI.
pub fn render_page(
    pdf_path: &Path,
    page_index: u32,
    dpi: u16,
    pdfium_lib: &Path,
) -> AppResult<image::DynamicImage> {
    let pdfium = load_pdfium(pdfium_lib)?;
    let doc = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| AppError::Config(format!("Failed to open PDF: {e}")))?;

    let page = doc
        .pages()
        .get(page_index as u16)
        .map_err(|e| AppError::Config(format!("Failed to get page {page_index}: {e}")))?;

    let render_config = PdfRenderConfig::new()
        .set_target_width((page.width().value as f64 * dpi as f64 / 72.0).round() as Pixels);

    let bitmap = page
        .render_with_config(&render_config)
        .map_err(|e| AppError::Config(format!("Failed to render page: {e}")))?;

    let img = bitmap.as_image();

    Ok(img)
}

/// Find first N non-blank page indices. A page is "blank" if rendering at low DPI
/// produces mean brightness > 250. Returns indices of first `count` content pages.
pub fn find_content_pages(pdf_path: &Path, count: usize, pdfium_lib: &Path) -> AppResult<Vec<u32>> {
    let pdfium = load_pdfium(pdfium_lib)?;
    let doc = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| AppError::Config(format!("Failed to open PDF: {e}")))?;

    let page_count = doc.pages().len() as u32;
    let mut result = Vec::with_capacity(count);

    for idx in 0..page_count {
        if result.len() >= count {
            break;
        }

        let page = match doc.pages().get(idx as u16) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Render at low resolution for quick blank detection
        let render_config = PdfRenderConfig::new().set_target_width(100);
        let bitmap = match page.render_with_config(&render_config) {
            Ok(b) => b,
            Err(_) => {
                // If rendering fails, assume non-blank
                result.push(idx);
                continue;
            }
        };

        let img = bitmap.as_image();
        let rgba = img.to_rgba8();
        let pixels = rgba.as_raw();
        if pixels.is_empty() {
            continue;
        }

        // Calculate mean brightness from RGBA pixels (use R, G, B, skip A)
        let mut total: u64 = 0;
        let mut count_pixels: u64 = 0;
        for chunk in pixels.chunks(4) {
            if chunk.len() >= 3 {
                total += (chunk[0] as u64 + chunk[1] as u64 + chunk[2] as u64) / 3;
                count_pixels += 1;
            }
        }
        let mean_brightness = if count_pixels > 0 {
            total / count_pixels
        } else {
            255
        };

        if mean_brightness <= 250 {
            result.push(idx);
        }
    }

    // If all pages were blank, just return first page(s)
    if result.is_empty() && page_count > 0 {
        for i in 0..count.min(page_count as usize) {
            result.push(i as u32);
        }
    }

    Ok(result)
}

/// Get the page count of a PDF without extracting text.
#[allow(dead_code)]
pub fn page_count(pdf_path: &Path, pdfium_lib: &Path) -> AppResult<u32> {
    let pdfium = load_pdfium(pdfium_lib)?;
    let doc = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| AppError::Config(format!("Failed to open PDF: {e}")))?;
    Ok(doc.pages().len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_scanned_pdf_detects_empty() {
        assert!(is_scanned_pdf("", 5));
    }

    #[test]
    fn is_scanned_pdf_detects_sparse_text() {
        // 5 pages, only 100 chars total = 20 avg per page < 50
        let text = "a".repeat(100);
        assert!(is_scanned_pdf(&text, 5));
    }

    #[test]
    fn is_scanned_pdf_accepts_rich_text() {
        // 2 pages, 500 chars = 250 avg per page > 50
        let text = "a".repeat(500);
        assert!(!is_scanned_pdf(&text, 2));
    }

    #[test]
    fn is_scanned_pdf_zero_pages() {
        assert!(is_scanned_pdf("some text", 0));
    }

    #[test]
    fn is_scanned_pdf_separator_not_counted() {
        // The separator "\n---\n" should be removed before counting
        let text = "short\n---\ntext";
        assert!(is_scanned_pdf(text, 2));
    }
}
