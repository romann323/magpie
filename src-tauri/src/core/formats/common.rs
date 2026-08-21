//! Utilities shared by every [`FormatHandler`]:
//!
//! - Atomic file replace (write to `<path>.<WRITE_TMP_SUFFIX>` and rename).
//! - EXIF → [`TechnicalMeta`] mapping (used by JPEG, PNG-with-eXIf, WebP,
//!   TIFF, HEIC, etc.).
//! - Image dimensions via the `image` crate.

pub use super::TechnicalMeta;
use crate::error::{AppError, AppResult};
use crate::paths::WRITE_TMP_SUFFIX;
use chrono::{NaiveDateTime, TimeZone, Utc};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// Remove Windows' extended-length ("verbatim") `\\?\` prefix from a path.
///
/// `std::fs::canonicalize` on Windows returns extended-length paths like
/// `\\?\C:\foo\bar` or `\\?\UNC\server\share\bar`. Those are fine for
/// `std::fs` (they're actually the *only* way to exceed `MAX_PATH`) but
/// several Win32 APIs reject them — most notably
/// `SHGetPropertyStoreFromParsingName`, which fails with
/// `E_INVALIDARG (0x80070057)` because the Shell name parser doesn't
/// understand the verbatim escape.
///
/// This helper mirrors what `dunce::simplified` does — undo the prefix
/// when the target path is safe to re-express (i.e. under `MAX_PATH` and
/// without illegal characters). If the resulting non-verbatim path would
/// be invalid we return the input unchanged.
pub fn strip_windows_verbatim_prefix<P: AsRef<Path>>(path: P) -> PathBuf {
    let s = path.as_ref().to_string_lossy();
    // UNC form: \\?\UNC\server\share\rest  →  \\server\share\rest
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        let simplified = format!(r"\\{rest}");
        if simplified.len() < 260 {
            return PathBuf::from(simplified);
        }
        return path.as_ref().to_path_buf();
    }
    // Drive form: \\?\C:\rest  →  C:\rest
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        // Rest must start with a drive letter + colon + separator to be a
        // valid non-verbatim path (rejects things like `\\?\Volume{...}`).
        let bytes = rest.as_bytes();
        let valid_drive = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/');
        if valid_drive && rest.len() < 260 {
            return PathBuf::from(rest);
        }
    }
    path.as_ref().to_path_buf()
}

#[cfg(test)]
mod strip_tests {
    use super::strip_windows_verbatim_prefix;
    use std::path::PathBuf;

    #[test]
    fn strips_drive_verbatim() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\C:\Users\romann\sample.x3f"),
            PathBuf::from(r"C:\Users\romann\sample.x3f")
        );
    }

    #[test]
    fn strips_unc_verbatim() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\UNC\server\share\file.jpg"),
            PathBuf::from(r"\\server\share\file.jpg")
        );
    }

    #[test]
    fn leaves_plain_path_alone() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"C:\normal\path.jpg"),
            PathBuf::from(r"C:\normal\path.jpg")
        );
    }

    #[test]
    fn refuses_volume_guid() {
        // Volume GUID paths can't be safely un-verbatim'd; leave them.
        let vg = r"\\?\Volume{12345678-1234-1234-1234-1234567890ab}\file.jpg";
        assert_eq!(strip_windows_verbatim_prefix(vg), PathBuf::from(vg));
    }
}

/// Atomic file replace: write to a temp file next to `path`, then rename over
/// the original. Renames within a single volume are atomic on both Windows
/// (via MoveFileEx replace-existing) and POSIX.
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> AppResult<()> {
    use std::io::Write;
    let tmp = {
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::MetadataWrite("no file name".into()))?
            .to_owned();
        let mut tmp_name = file_name;
        tmp_name.push(WRITE_TMP_SUFFIX);
        path.with_file_name(tmp_name)
    };
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| {
            AppError::MetadataWrite(format!("create {}: {e}", tmp.display()))
        })?;
        f.write_all(bytes)
            .map_err(|e| AppError::MetadataWrite(format!("write {}: {e}", tmp.display())))?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        AppError::MetadataWrite(format!(
            "rename {} → {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// Read pixel dimensions using the `image` crate. Returns `None` for formats
/// the crate doesn't understand or for corrupt files.
pub fn read_dimensions(path: &Path) -> Option<(u32, u32)> {
    let reader = image::ImageReader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    reader.into_dimensions().ok()
}

/// Collected EXIF facts we surface in both the DB (for filtering/sort) and
/// the DetailsPanel technical section.
#[derive(Default, Debug, Clone)]
pub struct ExifBits {
    pub taken_at: Option<i64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub iso: Option<String>,
    pub aperture: Option<String>,
    pub shutter: Option<String>,
    pub focal_length: Option<String>,
    pub gps: Option<String>,
    pub orientation: Option<String>,
}

/// Read every EXIF field Magpie surfaces from any file the `kamadak-exif`
/// crate can parse (JPEG, TIFF, PNG-with-eXIf, HEIC, WebP-with-EXIF).
///
/// Errors from the underlying reader are swallowed and converted to
/// `ExifBits::default()`; callers should treat "no EXIF" as normal.
pub fn read_exif(path: &Path) -> ExifBits {
    let Ok(f) = File::open(path) else {
        return ExifBits::default();
    };
    let mut br = BufReader::new(f);
    let exif_reader = exif::Reader::new();
    let Ok(exif) = exif_reader.read_from_container(&mut br) else {
        return ExifBits::default();
    };

    let mut out = ExifBits::default();

    // Date the photo was taken. Prefer DateTimeOriginal (when the picture was
    // captured) over DateTime (when it was last modified).
    if let Some(f) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        out.taken_at = parse_exif_datetime(&f.display_value().to_string());
    } else if let Some(f) = exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
        out.taken_at = parse_exif_datetime(&f.display_value().to_string());
    }

    out.camera_make = exif
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .map(|f| strip_quotes(&f.display_value().to_string()));
    out.camera_model = exif
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .map(|f| strip_quotes(&f.display_value().to_string()));
    out.lens = exif
        .get_field(exif::Tag::LensModel, exif::In::PRIMARY)
        .map(|f| strip_quotes(&f.display_value().to_string()));

    if let Some(f) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
        out.width = f.value.get_uint(0).map(|v| v as i64);
    }
    if let Some(f) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
        out.height = f.value.get_uint(0).map(|v| v as i64);
    }

    // Exposure triangle. `display_value()` returns nicely-formatted units
    // (`"1/125 s"`, `"f/1.8"`, etc.) so we render them verbatim.
    if let Some(f) = exif.get_field(exif::Tag::ISOSpeed, exif::In::PRIMARY) {
        out.iso = Some(strip_quotes(&f.display_value().to_string()));
    } else if let Some(f) = exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
        out.iso = Some(strip_quotes(&f.display_value().to_string()));
    }
    if let Some(f) = exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
        out.aperture = Some(strip_quotes(&f.display_value().to_string()));
    }
    if let Some(f) = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
        out.shutter = Some(strip_quotes(&f.display_value().to_string()));
    }
    if let Some(f) = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
        out.focal_length = Some(strip_quotes(&f.display_value().to_string()));
    }
    if let Some(f) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        out.orientation = Some(strip_quotes(&f.display_value().to_string()));
    }

    // GPS: latitude / longitude are stored as (deg, min, sec) triples; the
    // formatted display value already handles that.
    let lat = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY);
    let lat_ref = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY);
    let lon = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY);
    let lon_ref = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY);
    if let (Some(la), Some(lar), Some(lo), Some(lor)) = (lat, lat_ref, lon, lon_ref) {
        let s = format!(
            "{} {}, {} {}",
            strip_quotes(&la.display_value().to_string()),
            strip_quotes(&lar.display_value().to_string()),
            strip_quotes(&lo.display_value().to_string()),
            strip_quotes(&lor.display_value().to_string()),
        );
        out.gps = Some(s);
    }

    out
}

/// Append every non-empty EXIF field from `bits` to `tech` in the canonical
/// order used by the DetailsPanel.
pub fn append_exif_technical(tech: &mut TechnicalMeta, bits: &ExifBits) {
    if let Some(ts) = bits.taken_at {
        tech.push("Date taken", format_epoch_ms(ts));
    }
    let camera = [bits.camera_make.as_deref(), bits.camera_model.as_deref()]
        .iter()
        .filter_map(|s| s.map(str::trim).filter(|s| !s.is_empty()))
        .collect::<Vec<_>>()
        .join(" ");
    if !camera.is_empty() {
        tech.push("Camera", camera);
    }
    tech.push_opt("Lens", bits.lens.clone());
    tech.push_opt("ISO", bits.iso.clone());
    tech.push_opt("Aperture", bits.aperture.clone());
    tech.push_opt("Shutter", bits.shutter.clone());
    tech.push_opt("Focal length", bits.focal_length.clone());
    tech.push_opt("GPS", bits.gps.clone());
    tech.push_opt("Orientation", bits.orientation.clone());
}

pub fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

/// Parse `YYYY:MM:DD HH:MM:SS` (EXIF) into a UTC millisecond timestamp.
/// EXIF has no timezone so we treat the value as naive UTC.
pub fn parse_exif_datetime(s: &str) -> Option<i64> {
    let s = strip_quotes(s);
    let ndt = NaiveDateTime::parse_from_str(s.trim(), "%Y:%m:%d %H:%M:%S").ok()?;
    Some(Utc.from_utc_datetime(&ndt).timestamp_millis())
}

pub fn format_epoch_ms(ms: i64) -> String {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => String::new(),
    }
}

/// Formatted size string ("12.3 MB").
pub fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;
    let n = n as f64;
    if n < KB {
        format!("{} B", n as u64)
    } else if n < MB {
        format!("{:.1} KB", n / KB)
    } else if n < GB {
        format!("{:.1} MB", n / MB)
    } else {
        format!("{:.2} GB", n / GB)
    }
}

/// Standard "file info" preamble that every handler prepends — filename,
/// size, on-disk modified-at. Handler-specific rows go on top of this.
pub fn append_file_basics(tech: &mut TechnicalMeta, path: &Path) {
    if let Ok(m) = std::fs::metadata(path) {
        tech.push("Size on disk", format_bytes(m.len()));
        if let Ok(t) = m.modified() {
            if let Ok(d) = t.duration_since(std::time::UNIX_EPOCH) {
                tech.push("Modified", format_epoch_ms(d.as_millis() as i64));
            }
        }
    }
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        tech.push("Format", ext.to_ascii_uppercase());
    }
}

/// Error the read-only handlers return when a caller tries to persist tags.
pub fn write_not_supported_error(format_label: &str) -> AppError {
    AppError::MetadataWrite(format!(
        "{format_label} files can't yet store {} tags inside the file. \
         Supported for tag writing: JPEG, PNG, WebP, GIF89a.",
        crate::brand::PRODUCT_NAME,
    ))
}
