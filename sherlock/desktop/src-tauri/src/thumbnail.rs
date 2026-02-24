use std::path::{Path, PathBuf};

use crate::platform::paths::normalize_rel_path;

/// Result of thumbnail generation, including optional perceptual hash.
pub struct ThumbnailResult {
    pub path: String,
    /// dHash computed from the image. `Some` when the thumbnail was freshly
    /// generated, `None` when the cached thumbnail was reused (image not decoded).
    pub dhash: Option<u64>,
}

/// Compute a 64-bit difference hash (dHash) from a decoded image.
///
/// Resizes to 9x8 with a fast bilinear filter, converts to grayscale, and
/// compares adjacent horizontal pixels to produce a 64-bit hash.
pub fn compute_dhash(img: &image::DynamicImage) -> u64 {
    let small = img.resize_exact(9, 8, image::imageops::FilterType::Triangle);
    let gray = small.to_luma8();
    let mut hash: u64 = 0;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = gray.get_pixel(x, y).0[0];
            let right = gray.get_pixel(x + 1, y).0[0];
            if left > right {
                hash |= 1 << (y * 8 + x);
            }
        }
    }
    hash
}

/// Generate a thumbnail for the given source image.
///
/// Returns a `ThumbnailResult` with the path and optional dHash, or `None`
/// if generation fails. Skips regeneration if the thumbnail already exists
/// and the source hasn't changed (in which case `dhash` will be `None`).
pub fn generate_thumbnail(
    source_path: &Path,
    thumb_dir: &Path,
    rel_path: &str,
) -> Option<ThumbnailResult> {
    let stem = normalize_rel_path(&Path::new(rel_path).with_extension("jpg").to_string_lossy());
    let thumb_path = thumb_dir.join(&stem);

    // Skip if thumbnail already exists and source mtime hasn't changed
    if thumb_path.exists() {
        let source_mtime = std::fs::metadata(source_path)
            .ok()
            .and_then(|m| m.modified().ok());
        let thumb_mtime = std::fs::metadata(&thumb_path)
            .ok()
            .and_then(|m| m.modified().ok());
        if let (Some(s), Some(t)) = (source_mtime, thumb_mtime) {
            if t >= s {
                return Some(ThumbnailResult {
                    path: thumb_path.display().to_string(),
                    dhash: None,
                });
            }
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = thumb_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("Failed to create thumbnail dir {}: {e}", parent.display());
            return None;
        }
    }

    let effective_path = first_frame_if_gif(source_path);

    let img = match image::open(&effective_path) {
        Ok(img) => img,
        Err(e) => {
            log::warn!(
                "Failed to open image for thumbnail {}: {e}",
                source_path.display()
            );
            return None;
        }
    };

    let dhash = compute_dhash(&img);

    let max_dim = 300u32;
    let (w, h) = (img.width(), img.height());
    let resized = if w > max_dim || h > max_dim {
        let scale = max_dim as f64 / w.max(h) as f64;
        let new_w = (w as f64 * scale).round() as u32;
        let new_h = (h as f64 * scale).round() as u32;
        img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img // Already small enough, just re-encode as JPEG
    };

    let rgb = resized.to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
    if let Err(e) = rgb.write_with_encoder(encoder) {
        log::warn!("Failed to encode thumbnail: {e}");
        return None;
    }

    if let Err(e) = std::fs::write(&thumb_path, buf.into_inner()) {
        log::warn!("Failed to write thumbnail {}: {e}", thumb_path.display());
        return None;
    }

    Some(ThumbnailResult {
        path: thumb_path.display().to_string(),
        dhash: Some(dhash),
    })
}

/// Generate a thumbnail for a PDF: up to 2 content pages side-by-side, 300px max dimension.
pub fn generate_pdf_thumbnail(
    pdf_path: &Path,
    thumb_dir: &Path,
    rel_path: &str,
    pdfium_lib: &Path,
) -> Option<ThumbnailResult> {
    let stem = normalize_rel_path(&Path::new(rel_path).with_extension("jpg").to_string_lossy());
    let thumb_path = thumb_dir.join(&stem);

    // Skip if thumbnail already exists and source mtime hasn't changed
    if thumb_path.exists() {
        let source_mtime = std::fs::metadata(pdf_path)
            .ok()
            .and_then(|m| m.modified().ok());
        let thumb_mtime = std::fs::metadata(&thumb_path)
            .ok()
            .and_then(|m| m.modified().ok());
        if let (Some(s), Some(t)) = (source_mtime, thumb_mtime) {
            if t >= s {
                return Some(ThumbnailResult {
                    path: thumb_path.display().to_string(),
                    dhash: None,
                });
            }
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = thumb_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("Failed to create thumbnail dir {}: {e}", parent.display());
            return None;
        }
    }

    // Find up to 2 non-blank content pages
    let content_pages = match crate::pdf::find_content_pages(pdf_path, 2, pdfium_lib) {
        Ok(pages) => pages,
        Err(e) => {
            log::warn!(
                "Failed to find content pages in PDF {}: {e}",
                pdf_path.display()
            );
            return None;
        }
    };

    if content_pages.is_empty() {
        return None;
    }

    // Render each content page at moderate resolution (150px width target)
    let mut page_images: Vec<image::DynamicImage> = Vec::new();
    for &page_idx in &content_pages {
        match crate::pdf::render_page(pdf_path, page_idx, 96, pdfium_lib) {
            Ok(img) => {
                // Scale page to ~150px width
                let target_w = 150u32;
                let scale = target_w as f64 / img.width().max(1) as f64;
                let new_h = (img.height() as f64 * scale).round() as u32;
                let resized = img.resize(target_w, new_h, image::imageops::FilterType::Lanczos3);
                page_images.push(resized);
            }
            Err(e) => {
                log::warn!("Failed to render PDF page {page_idx}: {e}");
            }
        }
    }

    if page_images.is_empty() {
        return None;
    }

    // Stitch pages side-by-side if we have 2 pages
    let composite = if page_images.len() >= 2 {
        let gap = 2u32;
        let w = page_images[0].width() + gap + page_images[1].width();
        let h = page_images[0].height().max(page_images[1].height());
        let mut canvas = image::RgbImage::from_pixel(w, h, image::Rgb([40, 40, 40]));
        image::imageops::overlay(&mut canvas, &page_images[0].to_rgb8(), 0, 0);
        image::imageops::overlay(
            &mut canvas,
            &page_images[1].to_rgb8(),
            (page_images[0].width() + gap) as i64,
            0,
        );
        image::DynamicImage::ImageRgb8(canvas)
    } else {
        page_images.into_iter().next().unwrap()
    };

    // Scale composite so longest side is 300px
    let max_dim = 300u32;
    let (w, h) = (composite.width(), composite.height());
    let final_img = if w > max_dim || h > max_dim {
        let scale = max_dim as f64 / w.max(h) as f64;
        let new_w = (w as f64 * scale).round() as u32;
        let new_h = (h as f64 * scale).round() as u32;
        composite.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        composite
    };

    let dhash = compute_dhash(&final_img);

    let rgb = final_img.to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
    if let Err(e) = rgb.write_with_encoder(encoder) {
        log::warn!("Failed to encode PDF thumbnail: {e}");
        return None;
    }

    if let Err(e) = std::fs::write(&thumb_path, buf.into_inner()) {
        log::warn!(
            "Failed to write PDF thumbnail {}: {e}",
            thumb_path.display()
        );
        return None;
    }

    Some(ThumbnailResult {
        path: thumb_path.display().to_string(),
        dhash: Some(dhash),
    })
}

/// For GIF files, extract the first frame. For other formats, return as-is.
fn first_frame_if_gif(path: &Path) -> PathBuf {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    if ext.as_deref() != Some("gif") {
        return path.to_path_buf();
    }
    // For GIF, image::open already decodes the first frame
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image(dir: &Path, name: &str, width: u32, height: u32) -> PathBuf {
        let path = dir.join(name);
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let dynamic = image::DynamicImage::ImageRgb8(img);
        dynamic.save(&path).expect("save test image");
        path
    }

    #[test]
    fn generates_thumbnail_at_expected_path() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let source = create_test_image(src_dir.path(), "photo.png", 600, 400);
        let result = generate_thumbnail(&source, thumb_dir.path(), "subdir/photo.png");
        assert!(result.is_some());
        let tr = result.unwrap();
        let thumb_path = PathBuf::from(&tr.path);
        assert!(thumb_path.exists());
        assert!(thumb_path
            .display()
            .to_string()
            .ends_with("subdir/photo.jpg"));
        assert!(tr.dhash.is_some());

        // Verify the thumbnail is smaller
        let thumb_img = image::open(&thumb_path).expect("open thumb");
        assert!(thumb_img.width() <= 300);
    }

    #[test]
    fn skips_if_thumbnail_exists() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let source = create_test_image(src_dir.path(), "pic.png", 400, 300);

        let r1 = generate_thumbnail(&source, thumb_dir.path(), "pic.png");
        assert!(r1.is_some());
        assert!(r1.as_ref().unwrap().dhash.is_some());

        let thumb_path = PathBuf::from(&r1.unwrap().path);
        let mtime1 = std::fs::metadata(&thumb_path).unwrap().modified().unwrap();

        // Generate again - should skip (dhash will be None)
        std::thread::sleep(std::time::Duration::from_millis(50));
        let r2 = generate_thumbnail(&source, thumb_dir.path(), "pic.png");
        assert!(r2.is_some());
        assert!(r2.as_ref().unwrap().dhash.is_none());

        let mtime2 = std::fs::metadata(&thumb_path).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
    }

    #[test]
    fn handles_missing_source_gracefully() {
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let missing = std::env::temp_dir().join("nonexistent_image_12345.png");
        let result = generate_thumbnail(&missing, thumb_dir.path(), "missing.png");
        assert!(result.is_none());
    }

    #[test]
    fn tall_image_constrained_by_height() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let source = create_test_image(src_dir.path(), "tall.png", 200, 800);
        let result = generate_thumbnail(&source, thumb_dir.path(), "tall.png");
        assert!(result.is_some());
        let thumb_img = image::open(result.unwrap().path).expect("open");
        assert!(thumb_img.height() <= 300);
        assert!(thumb_img.width() < 200);
    }

    #[test]
    fn small_image_not_upscaled() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let source = create_test_image(src_dir.path(), "small.png", 100, 80);
        let result = generate_thumbnail(&source, thumb_dir.path(), "small.png");
        assert!(result.is_some());
        let thumb_img = image::open(result.unwrap().path).expect("open");
        assert_eq!(thumb_img.width(), 100);
    }

    #[test]
    fn dhash_stable_for_same_image() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let path = create_test_image(src_dir.path(), "stable.png", 200, 200);
        let img = image::open(&path).expect("open");
        let h1 = compute_dhash(&img);
        let h2 = compute_dhash(&img);
        assert_eq!(h1, h2);
    }

    #[test]
    fn dhash_different_for_different_images() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let path_a = create_test_image(src_dir.path(), "a.png", 200, 200);
        // Create a different image
        let path_b = src_dir.path().join("b.png");
        let img_b = image::RgbImage::from_fn(200, 200, |x, y| {
            image::Rgb([255 - (x % 256) as u8, (y % 256) as u8, 0])
        });
        image::DynamicImage::ImageRgb8(img_b).save(&path_b).unwrap();

        let img_a = image::open(&path_a).expect("open a");
        let img_b = image::open(&path_b).expect("open b");
        let ha = compute_dhash(&img_a);
        let hb = compute_dhash(&img_b);
        // Should be different (not guaranteed to be maximally different,
        // but with these patterns they should differ)
        assert_ne!(ha, hb);
    }
}
