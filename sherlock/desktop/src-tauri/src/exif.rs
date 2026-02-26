use std::path::Path;
use std::sync::OnceLock;

use reverse_geocoder::ReverseGeocoder;

static GEOCODER: OnceLock<ReverseGeocoder> = OnceLock::new();

fn geocoder() -> &'static ReverseGeocoder {
    GEOCODER.get_or_init(ReverseGeocoder::new)
}

/// Result of EXIF GPS extraction + reverse geocoding.
#[derive(Debug, Clone, Default)]
pub struct ExifLocation {
    #[allow(dead_code)] // Reserved for future map view
    pub latitude: Option<f64>,
    #[allow(dead_code)] // Reserved for future map view
    pub longitude: Option<f64>,
    pub location_text: String,
}

/// Detailed EXIF metadata for the properties dialog.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifDetails {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<String>,
    pub aperture: Option<String>,
    pub exposure_time: Option<String>,
    pub iso: Option<String>,
    pub date_taken: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub color_space: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub gps_location: Option<String>,
}

/// Extract detailed EXIF data from a file for the properties dialog.
pub fn extract_exif_details(path: &Path) -> ExifDetails {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ExifDetails::default(),
    };
    let mut buf_reader = std::io::BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut buf_reader) {
        Ok(e) => e,
        Err(_) => return ExifDetails::default(),
    };

    let get_str = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string().trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let get_u32 = |tag: exif::Tag| -> Option<u32> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| match &f.value {
                exif::Value::Long(v) => v.first().copied(),
                exif::Value::Short(v) => v.first().map(|x| *x as u32),
                _ => f.display_value().to_string().trim().parse::<u32>().ok(),
            })
    };

    // GPS extraction
    let (latitude, longitude) = extract_gps_coords(path)
        .map(|(lat, lon)| (Some(lat), Some(lon)))
        .unwrap_or((None, None));
    let gps_location = match (latitude, longitude) {
        (Some(lat), Some(lon)) => {
            let text = reverse_geocode(lat, lon);
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    };

    ExifDetails {
        camera_make: get_str(exif::Tag::Make),
        camera_model: get_str(exif::Tag::Model),
        lens_model: get_str(exif::Tag::LensModel),
        focal_length: get_str(exif::Tag::FocalLength),
        aperture: get_str(exif::Tag::FNumber),
        exposure_time: get_str(exif::Tag::ExposureTime),
        iso: get_str(exif::Tag::PhotographicSensitivity),
        date_taken: get_str(exif::Tag::DateTimeOriginal),
        image_width: get_u32(exif::Tag::PixelXDimension),
        image_height: get_u32(exif::Tag::PixelYDimension),
        color_space: get_str(exif::Tag::ColorSpace),
        latitude,
        longitude,
        gps_location,
    }
}

/// Read the EXIF Orientation tag (1-8). Returns 1 (normal) on any failure.
pub fn extract_orientation(path: &Path) -> u16 {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 1,
    };
    let mut buf_reader = std::io::BufReader::new(file);
    let exif_data = match exif::Reader::new().read_from_container(&mut buf_reader) {
        Ok(e) => e,
        Err(_) => return 1,
    };
    match exif_data.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        Some(field) => match &field.value {
            exif::Value::Short(v) => v.first().copied().unwrap_or(1),
            _ => 1,
        },
        None => 1,
    }
}

/// Apply EXIF orientation transform to a decoded image.
///
/// EXIF orientation values:
/// 1 = Normal, 2 = Flip H, 3 = Rotate 180, 4 = Flip V,
/// 5 = Transpose, 6 = Rotate 90 CW, 7 = Transverse, 8 = Rotate 270 CW
pub fn apply_orientation(img: image::DynamicImage, orientation: u16) -> image::DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.fliph().rotate90(),
        6 => img.rotate90(),
        7 => img.fliph().rotate270(),
        8 => img.rotate270(),
        _ => img, // 1 or unknown
    }
}

/// Main entry point: extract GPS coordinates from EXIF and reverse geocode.
pub fn extract_location(path: &Path) -> ExifLocation {
    let Some((lat, lon)) = extract_gps_coords(path) else {
        return ExifLocation::default();
    };
    let location_text = reverse_geocode(lat, lon);
    ExifLocation {
        latitude: Some(lat),
        longitude: Some(lon),
        location_text,
    }
}

/// Read EXIF GPS tags and convert DMS rational values to decimal degrees.
fn extract_gps_coords(path: &Path) -> Option<(f64, f64)> {
    let file = std::fs::File::open(path).ok()?;
    let mut buf_reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new()
        .read_from_container(&mut buf_reader)
        .ok()?;

    let lat_field = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)?;
    let lat_ref_field = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)?;
    let lon_field = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)?;
    let lon_ref_field = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)?;

    let lat = parse_dms_to_decimal(&lat_field.value)?;
    let lon = parse_dms_to_decimal(&lon_field.value)?;

    let lat_ref = lat_ref_field.display_value().to_string();
    let lon_ref = lon_ref_field.display_value().to_string();

    let lat = if lat_ref.contains('S') { -lat } else { lat };
    let lon = if lon_ref.contains('W') { -lon } else { lon };

    Some((lat, lon))
}

/// Convert EXIF DMS (degrees/minutes/seconds) rational values to decimal degrees.
fn parse_dms_to_decimal(value: &exif::Value) -> Option<f64> {
    match value {
        exif::Value::Rational(v) if v.len() >= 3 => {
            let degrees = v[0].to_f64();
            let minutes = v[1].to_f64();
            let seconds = v[2].to_f64();
            Some(degrees + minutes / 60.0 + seconds / 3600.0)
        }
        _ => None,
    }
}

/// Reverse geocode coordinates to a human-readable location string.
fn reverse_geocode(lat: f64, lon: f64) -> String {
    let result = geocoder().search((lat, lon));
    let record = result.record;
    let mut parts = Vec::new();
    if !record.name.is_empty() {
        parts.push(record.name.as_str());
    }
    if !record.admin1.is_empty() {
        parts.push(record.admin1.as_str());
    }
    if !record.cc.is_empty() {
        parts.push(record.cc.as_str());
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dms_rational_values() {
        // 40 degrees, 44 minutes, 54.36 seconds = ~40.748433
        let value = exif::Value::Rational(vec![
            exif::Rational { num: 40, denom: 1 },
            exif::Rational { num: 44, denom: 1 },
            exif::Rational {
                num: 5436,
                denom: 100,
            },
        ]);
        let result = parse_dms_to_decimal(&value).expect("should parse");
        assert!((result - 40.7484333).abs() < 0.001);
    }

    #[test]
    fn parse_dms_empty_value() {
        let value = exif::Value::Rational(vec![]);
        assert!(parse_dms_to_decimal(&value).is_none());
    }

    #[test]
    fn parse_dms_wrong_type() {
        let value = exif::Value::Ascii(vec![b"hello".to_vec()]);
        assert!(parse_dms_to_decimal(&value).is_none());
    }

    #[test]
    fn reverse_geocode_known_coords() {
        // NYC area (Times Square approx)
        let result = reverse_geocode(40.758, -73.9855);
        assert!(!result.is_empty());
        // Should contain US country code
        assert!(result.contains("US"));
    }

    #[test]
    fn extract_location_non_image() {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        std::fs::write(tmp.path(), b"not an image").expect("write");
        let loc = extract_location(tmp.path());
        assert!(loc.latitude.is_none());
        assert!(loc.longitude.is_none());
        assert!(loc.location_text.is_empty());
    }

    #[test]
    fn extract_location_nonexistent_file() {
        let loc = extract_location(Path::new("/nonexistent/file.jpg"));
        assert!(loc.latitude.is_none());
        assert!(loc.location_text.is_empty());
    }

    #[test]
    fn extract_orientation_nonexistent_file() {
        assert_eq!(extract_orientation(Path::new("/nonexistent.jpg")), 1);
    }

    #[test]
    fn extract_orientation_non_image() {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        std::fs::write(tmp.path(), b"not an image").expect("write");
        assert_eq!(extract_orientation(tmp.path()), 1);
    }

    #[test]
    fn apply_orientation_identity() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 2));
        let result = apply_orientation(img.clone(), 1);
        assert_eq!(result.width(), 4);
        assert_eq!(result.height(), 2);
    }

    #[test]
    fn apply_orientation_rotate90() {
        // Orientation 6 = rotate 90° CW: (4×2) becomes (2×4)
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 2));
        let result = apply_orientation(img, 6);
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 4);
    }

    #[test]
    fn apply_orientation_rotate270() {
        // Orientation 8 = rotate 270° CW: (4×2) becomes (2×4)
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 2));
        let result = apply_orientation(img, 8);
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 4);
    }

    #[test]
    fn apply_orientation_rotate180() {
        // Orientation 3 = rotate 180°: dimensions stay the same
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 2));
        let result = apply_orientation(img, 3);
        assert_eq!(result.width(), 4);
        assert_eq!(result.height(), 2);
    }

    #[test]
    fn apply_orientation_flip_h() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 2));
        let result = apply_orientation(img, 2);
        assert_eq!(result.width(), 4);
        assert_eq!(result.height(), 2);
    }
}
