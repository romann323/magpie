//! Metadata façade. Magpie's metadata pipeline is *read-only*: titles
//! and tags never round-trip back into the source file. On first scan,
//! whatever the file already carries — native XMP for JPEG/PNG/WebP/GIF,
//! Windows Shell property store for everything else — is imported into
//! `magpie.db`, which is the sole source of truth from that point on.
//!
//! Legacy Lightroom-style `.xmp` sidecars are still recognised on
//! first scan so pre-existing libraries import cleanly.

pub mod read;
pub mod sidecar;
