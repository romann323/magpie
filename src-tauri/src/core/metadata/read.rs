//! Metadata reader. Delegates to the [`FormatHandler`] registered for
//! the file's extension and layers a *legacy* sidecar-XMP override on
//! top so `.xmp` files written by an older Magpie build or by
//! Lightroom are still honoured on first scan.
//!
//! After the DB redesign this is only called once per file — on scan.
//! Whatever it returns is written into the per-folder library DB and
//! never re-read from disk again unless the file's mtime changes.

use crate::core::formats::{win_shell, xmp_packet, FormatRegistry};
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::db::queries::ImageMetaFromFile;
use crate::error::AppResult;
use std::path::Path;

pub fn read_all(registry: &FormatRegistry, path: &Path) -> AppResult<ImageMetaFromFile> {
    let mut out = ImageMetaFromFile::default();

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    if let Some(handler) = registry.for_ext(ext) {
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

        if out.width.is_none() || out.height.is_none() {
            if let Some((w, h)) = crate::core::formats::common::read_dimensions(path) {
                out.width = Some(w as i64);
                out.height = Some(h as i64);
            }
        }

        // Native handler user meta (XMP for JPEG/PNG/WebP/GIF, ...).
        if let Ok(um) = handler.read_user(path) {
            out.title = um.title;
            if !um.tags.is_empty() {
                out.tags = um.tags;
            }
        }

        // Windows Shell property system fallback: for any file whose
        // format handler has no native metadata reader, read whatever
        // Explorer's "Properties → Details" tab would show. This is how
        // pre-tagged RAW/HEIC/MP4/PDF libraries import cleanly on first
        // scan.
        //
        // Native XMP reads win over the Shell store because they surface
        // what the file itself embeds, not what the OS decided to cache.
        if out.title.is_none() || out.tags.is_empty() {
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
