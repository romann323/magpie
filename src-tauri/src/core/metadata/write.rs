//! Metadata writer. Persists the caller-supplied Title + Tags edits back into
//! the source file by delegating to the format handler for its extension,
//! then best-effort deletes any leftover legacy `.xmp` sidecar.
//!
//! On failure (unsupported format, read-only file, ...) the underlying
//! handler error is bubbled up — the UI surfaces it verbatim so the user
//! knows the save didn't land.

use crate::core::formats::{win_shell, FormatRegistry, UserMeta};
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::error::{AppError, AppResult};
use std::path::Path;

/// Persist the given `title` and `tags` to `path`. `None` on any field
/// means "leave it alone"; an empty tags vector means "clear all tags".
///
/// Dispatch order:
/// 1. If the format's [`FormatHandler`](crate::core::formats::FormatHandler)
///    can write tags natively (JPEG/PNG/WebP/GIF today), it's authoritative
///    and gets exclusive control over the file bytes.
/// 2. Otherwise, on Windows, fall back to the Shell property system
///    ([`win_shell`]). This is the exact same mechanism Explorer's
///    *Properties → Details* dialog uses, so anything the user tagged with
///    Windows is also visible to Magpie and vice-versa.
/// 3. If neither path is available we return a `MetadataWrite` error that
///    the UI surfaces verbatim.
pub fn write_metadata_to_source(
    registry: &FormatRegistry,
    path: &Path,
    title: Option<Option<String>>,
    tags: Option<Vec<String>>,
) -> AppResult<()> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let handler = registry.for_ext(&ext).ok_or_else(|| {
        AppError::MetadataWrite(format!(
            "'{}' files are not recognised by {}.",
            ext,
            crate::brand::PRODUCT_NAME
        ))
    })?;

    // Read existing user meta first so the read-modify-write cycle keeps
    // fields the caller didn't patch (title if only tags changed and vice
    // versa) and preserves foreign fields we don't surface (rating,
    // description). The XMP handlers do this preservation internally.
    // For non-writable handlers we consult the Shell store instead.
    let existing = if handler.can_write_tags() {
        handler.read_user(path).unwrap_or_default()
    } else {
        win_shell::read_user_meta(path).unwrap_or_default()
    };

    let mut edits = UserMeta {
        title: existing.title,
        tags: existing.tags,
    };
    if let Some(t) = title {
        edits.title = t;
    }
    if let Some(new_tags) = tags {
        edits.tags = new_tags;
    }

    // The `path` we've been given comes straight from `images.path` in the
    // DB — set by the folder scanner from the file's absolute location on
    // disk. Log it explicitly so anyone auditing the write pipeline can
    // confirm nothing points at a thumbnail cache.
    tracing::info!(source = %path.display(), ext, "writing metadata to source file");

    if handler.can_write_tags() {
        handler.write_user(path, &edits)?;
        tracing::info!(?path, handler = handler.name(), "embedded metadata (native)");
    } else {
        // Windows Shell fallback. On non-Windows this returns an error
        // explaining the platform gap.
        win_shell::write_user_meta(path, &edits).map_err(|e| {
            // Enrich the error with the actual extension so the UI can guide
            // the user (e.g. "install Sigma Photo Pro's property handler").
            AppError::MetadataWrite(format!(
                "Couldn't save tags to '{}' ({}). {}",
                path.display(),
                ext,
                trim_leading_prefix(&e.to_string())
            ))
        })?;
        tracing::info!(?path, handler = handler.name(), "embedded metadata (Windows Shell)");
    }

    // Clean up any leftover legacy `.xmp` sidecar. Best-effort — failure to
    // delete is logged but doesn't fail the save (the source file has the
    // metadata now, so nothing is lost).
    let sidecar = sidecar_path_for(path);
    if sidecar.exists() {
        match std::fs::remove_file(&sidecar) {
            Ok(()) => tracing::info!(?sidecar, "removed legacy XMP sidecar after embed"),
            Err(e) => tracing::warn!(?sidecar, error = %e, "could not remove legacy sidecar"),
        }
    }
    Ok(())
}

/// Strip the leading `"metadata write error: "` (or similar) from a
/// bubbled-up [`AppError`] display so the outer message doesn't say "error:
/// error: ...".
fn trim_leading_prefix(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix("metadata write error: ") {
        rest
    } else {
        s
    }
}
