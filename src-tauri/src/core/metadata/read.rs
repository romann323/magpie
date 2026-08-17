use crate::core::is_processable_by_image_crate;
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::core::metadata::xmp;
use crate::db::queries::ImageMetaFromFile;
use crate::error::{PicOrgError, PicOrgResult};
use chrono::{NaiveDateTime, TimeZone, Utc};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Reads all recoverable metadata from an image on disk:
/// - Basic EXIF (dimensions, DateTimeOriginal, camera).
/// - XMP embedded in the file (for JPEG/PNG/TIFF/WebP/HEIC when supported).
/// - XMP sidecar file (Lightroom-style) if present, which OVERRIDES embedded values
///   for the "user metadata" fields (title, rating, tags, comment).
pub fn read_all(path: &Path) -> PicOrgResult<ImageMetaFromFile> {
    let mut out = ImageMetaFromFile::default();

    if is_processable_by_image_crate(path.extension().and_then(|s| s.to_str()).unwrap_or("")) {
        if let Ok((w, h)) = read_dimensions(path) {
            out.width = Some(w as i64);
            out.height = Some(h as i64);
        }
    }

    // EXIF
    if let Ok(exif_data) = read_exif(path) {
        if let Some(dt) = exif_data.taken_at {
            out.taken_at = Some(dt);
        }
        out.camera_make = exif_data.camera_make;
        out.camera_model = exif_data.camera_model;
        if out.width.is_none() {
            out.width = exif_data.width;
        }
        if out.height.is_none() {
            out.height = exif_data.height;
        }
    }

    // Embedded XMP (for supported formats — JPEG most reliably)
    if let Some(bytes) = xmp::extract_embedded_xmp(path).ok().flatten() {
        if let Ok(user) = xmp::parse_user_metadata(&bytes) {
            apply_user_meta(&mut out, user);
        }
    }

    // Sidecar XMP: takes precedence for user metadata (Lightroom convention)
    let sidecar = sidecar_path_for(path);
    if sidecar.exists() {
        if let Ok(bytes) = std::fs::read(&sidecar) {
            if let Ok(user) = xmp::parse_user_metadata(&bytes) {
                apply_user_meta(&mut out, user);
            }
        }
    }

    Ok(out)
}

fn apply_user_meta(out: &mut ImageMetaFromFile, user: xmp::UserMetadata) {
    if user.title.is_some() {
        out.title = user.title;
    }
    if user.rating.is_some() {
        out.rating = user.rating;
    }
    if user.description.is_some() {
        out.comment = user.description;
    }
    if let Some(tags) = user.subjects {
        if !tags.is_empty() {
            out.tags = tags;
        }
    }
}

fn read_dimensions(path: &Path) -> PicOrgResult<(u32, u32)> {
    let reader = image::ImageReader::open(path)
        .map_err(|e| PicOrgError::ImageDecode(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| PicOrgError::ImageDecode(e.to_string()))?;
    reader
        .into_dimensions()
        .map_err(|e| PicOrgError::ImageDecode(e.to_string()))
}

#[derive(Default)]
struct ExifBits {
    taken_at: Option<i64>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
}

fn read_exif(path: &Path) -> PicOrgResult<ExifBits> {
    let f = File::open(path)?;
    let mut br = BufReader::new(f);
    let exifreader = exif::Reader::new();
    let exif = exifreader
        .read_from_container(&mut br)
        .map_err(|e| PicOrgError::MetadataRead(format!("exif: {e}")))?;

    let mut out = ExifBits::default();

    if let Some(f) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        if let Some(ts) = parse_exif_datetime(&f.display_value().to_string()) {
            out.taken_at = Some(ts);
        }
    } else if let Some(f) = exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
        if let Some(ts) = parse_exif_datetime(&f.display_value().to_string()) {
            out.taken_at = Some(ts);
        }
    }

    if let Some(f) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
        out.camera_make = Some(strip_quotes(&f.display_value().to_string()));
    }
    if let Some(f) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        out.camera_model = Some(strip_quotes(&f.display_value().to_string()));
    }
    if let Some(f) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
        out.width = f.value.get_uint(0).map(|v| v as i64);
    }
    if let Some(f) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
        out.height = f.value.get_uint(0).map(|v| v as i64);
    }

    Ok(out)
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

/// Parses `YYYY:MM:DD HH:MM:SS` (EXIF format) into a UTC-ish millisecond timestamp.
/// EXIF does not store a timezone; we treat the value as local naive time.
fn parse_exif_datetime(s: &str) -> Option<i64> {
    let s = strip_quotes(s);
    let ndt = NaiveDateTime::parse_from_str(s.trim(), "%Y:%m:%d %H:%M:%S").ok()?;
    Some(Utc.from_utc_datetime(&ndt).timestamp_millis())
}
