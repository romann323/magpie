//! Metadata reader. Delegates to the [`FormatHandler`] registered for the
//! file's extension and layers a *legacy* sidecar-XMP override on top so
//! `.xmp` files written by an older Magpie build or by Lightroom are still
//! honoured on first scan.

use crate::core::formats::{win_shell, xmp_packet, FormatRegistry};
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::db::queries::ImageMetaFromFile;
use crate::error::AppResult;
use std::path::Path;

/// Read every recoverable metadata field for a file:
/// - Handler-provided technical metadata (dimensions, EXIF, GPS, ...)
///   summarized down to the DB-persisted subset (dimensions/taken_at/camera).
/// - Handler-provided user metadata (title, tags).
/// - Legacy `.xmp` sidecar override — if a sidecar exists it wins over the
///   in-file metadata, so old Lightroom-authored `.xmp` files are respected
///   on the first scan. The first successful write clears the sidecar.
pub fn read_all(
    registry: &FormatRegistry,
    path: &Path,
) -> AppResult<ImageMetaFromFile> {
    let mut out = ImageMetaFromFile::default();

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    if let Some(handler) = registry.for_ext(ext) {
        // Handler-side technical metadata → DB-persisted subset. We currently
        // extract only the fields the DB indexes (dimensions, taken_at,
        // camera make/model). The full TechnicalMeta is served on-demand by
        // commands/images.rs for the DetailsPanel.
        let bits = crate::core::formats::common::read_exif(path);
        if let Some(w) = bits.width {
            out.width = Some(w);
        }
        if let Some(h) = bits.height {
            out.height = Some(h);
        }
        if let Some(t) = bits.taken_at {
            out.taken_at = Some(t);
        }
        out.camera_make = bits.camera_make;
        out.camera_model = bits.camera_model;

        // Handlers that decode dimensions themselves (JPEG, PNG, WebP, GIF,
        // BMP) give more reliable numbers than EXIF's PixelXDimension.
        if out.width.is_none() || out.height.is_none() {
            if let Some((w, h)) = crate::core::formats::common::read_dimensions(path) {
                out.width = Some(w as i64);
                out.height = Some(h as i64);
            }
        }

        // Handler user meta.
        if let Ok(um) = handler.read_user(path) {
            out.title = um.title;
            if !um.tags.is_empty() {
                out.tags = um.tags;
            }
        }

        // Shell fallback: for handlers that can't natively embed tags, ask
        // Windows for whatever Explorer's Properties → Details tab may have
        // stored. Native XMP writers (JPEG/PNG/WebP/GIF) are authoritative
        // and skipped here so we never fight our own writes.
        if !handler.can_write_tags() {
            if let Some(shell_um) = win_shell::read_user_meta(path) {
                if out.title.is_none() {
                    out.title = shell_um.title;
                }
                if out.tags.is_empty() && !shell_um.tags.is_empty() {
                    out.tags = shell_um.tags;
                }
            }
        }
    }

    // Legacy sidecar override (Lightroom `.xmp` next to the image).
    let sidecar = sidecar_path_for(path);
    if sidecar.exists() {
        if let Ok(bytes) = std::fs::read(&sidecar) {
            if let Ok(x) = xmp_packet::parse_xmp(&bytes) {
                if let Some(t) = x.title {
                    out.title = Some(t);
                }
                if let Some(tags) = x.subjects {
                    if !tags.is_empty() {
                        out.tags = tags;
                    }
                }
            }
        }
    }

    Ok(out)
}
