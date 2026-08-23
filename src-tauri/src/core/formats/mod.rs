//! Pluggable per-format handlers.
//!
//! After the DB redesign, format handlers are **read-only**. Tags and
//! titles are stored in the per-folder library DB, not in the file
//! bytes. Handlers exist to:
//!
//! 1. Extract *technical* metadata (dimensions, EXIF, duration, page
//!    count, GPS) for display in the DetailsPanel and for filtering
//!    (`taken_at`, `camera_*`).
//! 2. Read whatever tags a file *already* carries on first scan (JPEG
//!    XMP, PNG iTXt, Lightroom `.xmp` sidecar, Windows Shell property
//!    store) so pre-existing libraries import cleanly.
//!
//! Adding support for a new file type is a matter of:
//!
//! 1. Adding a file under `src-tauri/src/core/formats/` implementing
//!    [`FormatHandler`].
//! 2. Registering it in [`FormatRegistry::new`].
//!
//! The scanner iterates every registered extension via
//! [`FormatRegistry::all_extensions`], so registering a handler makes
//! its files immediately visible in the library.

use crate::error::AppResult;
use std::path::Path;

pub mod xmp_packet;
pub mod common;
pub mod win_shell;

mod gif;
mod jpeg;
mod png;
mod stubs;
mod tiff;
mod webp;

/// Ordered `(label, value)` pairs of read-only technical metadata that the
/// DetailsPanel renders in its "File info" section. Handlers append entries
/// in a stable order (Dimensions before EXIF, Camera before Lens, etc.).
#[derive(Debug, Default, Clone)]
pub struct TechnicalMeta {
    pub entries: Vec<(String, String)>,
}

impl TechnicalMeta {
    pub fn push(&mut self, key: &str, value: impl Into<String>) {
        let v = value.into();
        if !v.trim().is_empty() {
            self.entries.push((key.to_string(), v));
        }
    }

    pub fn push_opt<S: Into<String>>(&mut self, key: &str, value: Option<S>) {
        if let Some(v) = value {
            self.push(key, v);
        }
    }

    pub fn as_pairs(&self) -> Vec<[String; 2]> {
        self.entries
            .iter()
            .map(|(k, v)| [k.clone(), v.clone()])
            .collect()
    }
}

/// Subset of on-disk metadata that Magpie *edits*. Rating and description
/// slots exist in the underlying XMP schema but Magpie no longer surfaces
/// them; format handlers preserve those foreign fields on write without
/// touching them (see [`xmp_packet::XmpUserMeta`]).
#[derive(Debug, Default, Clone)]
pub struct UserMeta {
    pub title: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Image,
    Video,
    Document,
    Other,
}

/// The behaviour of one file format (JPEG, PNG, MP4, ...).
///
/// # Contract
///
/// - `read_technical` must never panic on garbage input. It returns an empty
///   [`TechnicalMeta`] instead.
/// - `read_user` may return `Err` for genuine I/O errors, but "file has no
///   metadata slot" is `Ok(UserMeta::default())`.
pub trait FormatHandler: Send + Sync {
    /// Short lowercase identifier surfaced to the UI (e.g. `"jpeg"`).
    fn name(&self) -> &'static str;

    /// Lowercased extensions this handler answers to. No leading dot.
    fn extensions(&self) -> &'static [&'static str];

    fn kind(&self) -> FormatKind;

    fn read_technical(&self, path: &Path) -> TechnicalMeta;

    fn read_user(&self, path: &Path) -> AppResult<UserMeta>;
}

/// Runtime lookup table from file extension to [`FormatHandler`]. Built once
/// in [`AppServices::new`] and shared across every scan/read/write.
pub struct FormatRegistry {
    handlers: Vec<Box<dyn FormatHandler>>,
    by_ext: std::collections::HashMap<String, usize>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        let mut r = FormatRegistry {
            handlers: Vec::new(),
            by_ext: std::collections::HashMap::new(),
        };

        // Full read+write handlers first — they get priority when the same
        // extension is claimed by more than one (shouldn't happen in
        // practice; leftmost registration wins).
        r.register(Box::new(jpeg::JpegHandler));
        r.register(Box::new(png::PngHandler));
        r.register(Box::new(webp::WebpHandler));
        r.register(Box::new(gif::GifHandler));

        // Read-only for now — a future PR can promote them.
        r.register(Box::new(tiff::TiffHandler));

        for h in stubs::all_stubs() {
            r.register(h);
        }

        r
    }

    fn register(&mut self, h: Box<dyn FormatHandler>) {
        let idx = self.handlers.len();
        for ext in h.extensions() {
            let key = ext.to_ascii_lowercase();
            self.by_ext.entry(key).or_insert(idx);
        }
        self.handlers.push(h);
    }

    pub fn for_ext(&self, ext: &str) -> Option<&dyn FormatHandler> {
        let e = ext.to_ascii_lowercase();
        self.by_ext.get(&e).map(|&i| self.handlers[i].as_ref())
    }

    /// Sorted list of every extension the registry recognises. Consumed by
    /// the scanner to pick up files during folder walks.
    pub fn all_extensions(&self) -> Vec<String> {
        let mut v: Vec<String> = self.by_ext.keys().cloned().collect();
        v.sort();
        v
    }

    /// Does the registry recognise this extension? Case-insensitive.
    pub fn is_recognized(&self, ext: &str) -> bool {
        let e = ext.to_ascii_lowercase();
        self.by_ext.contains_key(&e)
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}
