//! Metadata façade. Delegates every read/write to the [`FormatRegistry`]
//! (see `crate::core::formats`). Also handles the *legacy* Lightroom-style
//! `.xmp` sidecar file discovery so old libraries migrate cleanly on their
//! first save.

pub mod read;
pub mod sidecar;
pub mod write;
