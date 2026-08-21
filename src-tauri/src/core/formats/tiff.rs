//! TIFF / DNG format handler (read-only for the tag-write path).
//!
//! Reads dimensions and EXIF (which lives directly in the TIFF IFDs — no
//! separate parser needed) and also reads any existing XMP packet in tag
//! 700 so we can surface pre-existing titles/tags to the user. Writing tags
//! back into TIFF requires safely rebuilding the IFD chain (each IFD entry
//! offset would shift if we resize tag 700), which is out of scope for this
//! milestone; see the top-level `formats` module docs for the plan.

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::AppResult;
use std::path::Path;

/// TIFF's XMP tag number (per Adobe XMP Specification Part 3).
const TAG_XMP: u16 = 700;

pub struct TiffHandler;

impl FormatHandler for TiffHandler {
    fn name(&self) -> &'static str {
        "tiff"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tif", "tiff", "dng"]
    }

    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }

    fn can_write_tags(&self) -> bool {
        false
    }

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        if let Some((w, h)) = common::read_dimensions(path) {
            tech.push("Dimensions", format!("{w} × {h} px"));
        }
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
        common::append_file_basics(&mut tech, path);
        tech
    }

    fn read_user(&self, path: &Path) -> AppResult<UserMeta> {
        match extract_xmp(path) {
            Some(bytes) => {
                let x = xmp_packet::parse_xmp(&bytes)?;
                Ok(xmp_packet::to_user_meta(&x))
            }
            None => Ok(UserMeta::default()),
        }
    }

    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(
            "TIFF / DNG",
        ))
    }
}

/// Extract the XMP packet from tag 700 of the primary TIFF IFD, if any.
///
/// The parser is deliberately narrow — we only need to locate one tag, so
/// we walk the IFD entries linearly and match on tag id 700. Errors from
/// truncated / malformed TIFFs return `None` rather than propagating.
fn extract_xmp(path: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    // Byte-order marker: "II" = little-endian, "MM" = big-endian.
    let le = match &bytes[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    // Magic 42
    let magic = read_u16(&bytes, 2, le);
    if magic != 42 {
        return None;
    }
    let ifd_off = read_u32(&bytes, 4, le) as usize;
    if ifd_off + 2 > bytes.len() {
        return None;
    }
    let entry_count = read_u16(&bytes, ifd_off, le) as usize;
    let entries_start = ifd_off + 2;
    if entries_start + entry_count * 12 > bytes.len() {
        return None;
    }
    for i in 0..entry_count {
        let entry = entries_start + i * 12;
        let tag = read_u16(&bytes, entry, le);
        if tag != TAG_XMP {
            continue;
        }
        // let field_type = read_u16(&bytes, entry + 2, le); // 1=BYTE, 7=UNDEFINED
        let count = read_u32(&bytes, entry + 4, le) as usize;
        let value_or_offset = read_u32(&bytes, entry + 8, le) as usize;

        // For type BYTE/UNDEFINED, if `count <= 4` the value is stored
        // inline in the 4-byte field; otherwise it's at `value_or_offset`.
        let (start, end) = if count <= 4 {
            (entry + 8, entry + 8 + count)
        } else {
            (value_or_offset, value_or_offset + count)
        };
        if end > bytes.len() {
            return None;
        }
        return Some(bytes[start..end].to_vec());
    }
    None
}

fn read_u16(bytes: &[u8], off: usize, le: bool) -> u16 {
    if le {
        u16::from_le_bytes([bytes[off], bytes[off + 1]])
    } else {
        u16::from_be_bytes([bytes[off], bytes[off + 1]])
    }
}

fn read_u32(bytes: &[u8], off: usize, le: bool) -> u32 {
    if le {
        u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
    } else {
        u32::from_be_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
    }
}
