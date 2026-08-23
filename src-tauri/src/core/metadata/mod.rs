//! Metadata façade. After the DB redesign this module is *read-only*:
//! titles and tags never round-trip back into the source file. Magpie
//! reads whatever the file already carries on first scan (native XMP
//! for JPEG/PNG/WebP/GIF, Windows Shell property store for everything
//! else), imports it into the per-folder library DB, and treats the DB
//! as the sole source of truth from then on.
//!
//! Legacy Lightroom-style `.xmp` sidecars are still recognised on
//! first scan so pre-existing libraries import cleanly.

pub mod read;
pub mod sidecar;
