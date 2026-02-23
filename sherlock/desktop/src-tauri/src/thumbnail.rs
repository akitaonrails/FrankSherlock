use std::path::{Path, PathBuf};

/// Generate a thumbnail for the given source image.
///
/// Returns the absolute path to the thumbnail, or `None` if generation fails.
/// Skips regeneration if the thumbnail already exists and the source hasn't changed.
pub fn generate_thumbnail(source_path: &Path, thumb_dir: &Path, rel_path: &str) -> Option<String> {
    let stem = Path::new(rel_path)
        .with_extension("jpg")
        .to_string_lossy()
        .replace('\\', "/");
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
                return Some(thumb_path.display().to_string());
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

    Some(thumb_path.display().to_string())
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
        let thumb_path = PathBuf::from(result.unwrap());
        assert!(thumb_path.exists());
        assert!(thumb_path
            .display()
            .to_string()
            .ends_with("subdir/photo.jpg"));

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

        let thumb_path = PathBuf::from(r1.as_ref().unwrap());
        let mtime1 = std::fs::metadata(&thumb_path).unwrap().modified().unwrap();

        // Generate again - should skip
        std::thread::sleep(std::time::Duration::from_millis(50));
        let r2 = generate_thumbnail(&source, thumb_dir.path(), "pic.png");
        assert!(r2.is_some());

        let mtime2 = std::fs::metadata(&thumb_path).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
    }

    #[test]
    fn handles_missing_source_gracefully() {
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let missing = Path::new("/tmp/nonexistent_image_12345.png");
        let result = generate_thumbnail(missing, thumb_dir.path(), "missing.png");
        assert!(result.is_none());
    }

    #[test]
    fn tall_image_constrained_by_height() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let thumb_dir = tempfile::tempdir().expect("tempdir");
        let source = create_test_image(src_dir.path(), "tall.png", 200, 800);
        let result = generate_thumbnail(&source, thumb_dir.path(), "tall.png");
        assert!(result.is_some());
        let thumb_img = image::open(result.unwrap()).expect("open");
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
        let thumb_img = image::open(result.unwrap()).expect("open");
        assert_eq!(thumb_img.width(), 100);
    }
}
