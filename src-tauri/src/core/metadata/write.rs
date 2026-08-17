//! Writing user metadata to disk.
//!
//! v1 policy: always write an XMP **sidecar** file next to the image.
//! This is universally safe (never mutates the original), works for every
//! input format (JPEG, PNG, HEIC, RAW, TIFF, WebP…), and is the same
//! convention Adobe Lightroom uses for RAW files.

use crate::core::metadata::read as meta_read;
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::core::metadata::xmp::{build_xmp_packet, embed_xmp_in_source, UserMetadata};
use crate::error::{PicOrgError, PicOrgResult};
use std::io::Write;
use std::path::Path;

/// Write the given user metadata to the sidecar file for `image_path`.
/// This is atomic on Windows: writes to `sidecar.tmp` then renames.
pub fn write_sidecar(image_path: &Path, meta: &UserMetadata) -> PicOrgResult<()> {
    let sidecar = sidecar_path_for(image_path);
    let tmp = sidecar.with_extension("xmp.tmp");

    let xml = build_xmp_packet(meta);

    // Atomic replace: write to .tmp, then rename over the target.
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| {
            PicOrgError::MetadataWrite(format!("create {}: {e}", tmp.display()))
        })?;
        f.write_all(xml.as_bytes())
            .map_err(|e| PicOrgError::MetadataWrite(format!("write {}: {e}", tmp.display())))?;
        f.sync_all().ok();
    }
    // On Windows, `rename` will fail if the target exists — use a shim.
    replace_file(&tmp, &sidecar).map_err(|e| {
        PicOrgError::MetadataWrite(format!(
            "rename {} → {}: {e}",
            tmp.display(),
            sidecar.display()
        ))
    })?;

    Ok(())
}

/// Read current metadata from disk, apply changes, write it back.
///
/// Two-pronged persistence:
/// 1. **Sidecar `.xmp`** next to the source (Lightroom-standard, universal).
/// 2. **Embedded XMP inside the source file** (JPEG/PNG) so tools that don't
///    read sidecars — Windows Explorer, Photos app, most viewers — also see
///    the tags/title/rating. Formats we can't embed into (HEIC, RAW, TIFF,
///    WebP…) still get the sidecar.
///
/// If embedding fails (e.g. the source file is read-only or on a mount that
/// forbids rewrites), we still return `Ok(())` after writing the sidecar so
/// the user's edit isn't lost. The embed error is surfaced via `tracing::warn`.
pub fn merge_and_write_sidecar(
    image_path: &Path,
    patch_title: Option<Option<String>>,
    patch_description: Option<Option<String>>,
    patch_rating: Option<Option<i64>>,
    patch_subjects: Option<Vec<String>>,
) -> PicOrgResult<()> {
    let existing = meta_read::read_all(image_path).ok();

    let mut m = UserMetadata::default();
    if let Some(e) = existing {
        m.title = e.title;
        m.description = e.comment;
        m.rating = e.rating;
        if !e.tags.is_empty() {
            m.subjects = Some(e.tags);
        }
    }

    if let Some(t) = patch_title {
        m.title = t;
    }
    if let Some(d) = patch_description {
        m.description = d;
    }
    if let Some(r) = patch_rating {
        m.rating = r;
    }
    if let Some(s) = patch_subjects {
        m.subjects = if s.is_empty() { None } else { Some(s) };
    }

    // Always write the sidecar first (safe, universal fallback).
    write_sidecar(image_path, &m)?;

    // Then try to embed into the source file itself. For formats we don't
    // support this returns Ok(false); for supported formats an error is
    // logged but doesn't fail the whole save (the sidecar is enough).
    let xmp_bytes = build_xmp_packet(&m).into_bytes();
    match embed_xmp_in_source(image_path, &xmp_bytes) {
        Ok(true) => {
            tracing::info!(?image_path, "embedded XMP into source file");
        }
        Ok(false) => {
            tracing::debug!(?image_path, "format does not support embedded XMP; sidecar only");
        }
        Err(e) => {
            tracing::warn!(?image_path, error = %e, "embedded XMP write failed; sidecar still written");
        }
    }
    Ok(())
}

#[cfg(windows)]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    // std::fs::rename on Windows replaces if the destination is a file.
    std::fs::rename(src, dst)
}

#[cfg(not(windows))]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::rename(src, dst)
}
