//! JPEG format handler.
//!
//! Reads/writes the standard XMP APP1 segment (`http://ns.adobe.com/xap/1.0/`).
//! Existing standard-XMP and ExtendedXMP APP1 segments are dropped before the
//! new one is inserted so we don't stack duplicates.

use super::common::{self, ExifBits, TechnicalMeta};
use super::xmp_packet;
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::io::Read;
use std::path::Path;

const XMP_MARKER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const XMP_EXT_MARKER: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";
/// APP1 segment length field is `u16` big-endian, so payload max is
/// `65535 - 2 == 65533` bytes. Adobe defines ExtendedXMP for larger packets,
/// but Magpie's own packets are always well under 4 KB.
const MAX_SEGMENT_PAYLOAD: usize = 65533;

pub struct JpegHandler;

impl FormatHandler for JpegHandler {
    fn name(&self) -> &'static str {
        "jpeg"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["jpg", "jpeg", "jpe", "jfif", "jif"]
    }

    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }

    fn can_write_tags(&self) -> bool {
        true
    }

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        let (w, h) = common::read_dimensions(path)
            .or_else(|| {
                // Fall back to EXIF's PixelXDimension for exotic JPEGs the
                // `image` crate refuses to decode.
                let bits = common::read_exif(path);
                match (bits.width, bits.height) {
                    (Some(w), Some(h)) => Some((w as u32, h as u32)),
                    _ => None,
                }
            })
            .unzip();
        if let (Some(w), Some(h)) = (w, h) {
            tech.push("Dimensions", format!("{w} × {h} px"));
        }
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
        common::append_file_basics(&mut tech, path);
        tech
    }

    fn read_user(&self, path: &Path) -> AppResult<UserMeta> {
        match extract_xmp(path)? {
            Some(bytes) => {
                let x = xmp_packet::parse_xmp(&bytes)?;
                Ok(xmp_packet::to_user_meta(&x))
            }
            None => Ok(UserMeta::default()),
        }
    }

    fn write_user(&self, path: &Path, edits: &UserMeta) -> AppResult<()> {
        let existing = extract_xmp(path)
            .ok()
            .flatten()
            .and_then(|b| xmp_packet::parse_xmp(&b).ok())
            .unwrap_or_default();
        let merged = xmp_packet::merge_user_edits(existing, edits);
        let xmp = xmp_packet::build_xmp_packet(&merged);
        embed_xmp(path, xmp.as_bytes())
    }
}

/// Read the JPEG file's leading header (up to 2 MiB) and hand back the XMP
/// packet bytes if an XMP APP1 segment is present.
fn extract_xmp(path: &Path) -> AppResult<Option<Vec<u8>>> {
    let mut f = std::fs::File::open(path)
        .map_err(|e| AppError::MetadataRead(format!("open {}: {e}", path.display())))?;
    let mut buf = Vec::with_capacity(64 * 1024);
    let cap = 2 * 1024 * 1024;
    let n = (&mut f)
        .take(cap as u64)
        .read_to_end(&mut buf)
        .map_err(|e| AppError::MetadataRead(format!("read {}: {e}", path.display())))?;
    let data = &buf[..n];

    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Ok(None);
    }
    let mut i = 2usize;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            break;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            break;
        }
        let marker = data[i];
        i += 1;
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if i + 2 > data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            break;
        }
        let payload = &data[i + 2..i + seg_len];
        i += seg_len;
        if marker == 0xE1 && payload.starts_with(XMP_MARKER) {
            let start = XMP_MARKER.len();
            return Ok(Some(payload[start..].to_vec()));
        }
        if marker == 0xDA {
            break;
        }
    }
    Ok(None)
}

fn embed_xmp(path: &Path, xmp_bytes: &[u8]) -> AppResult<()> {
    let orig = std::fs::read(path)
        .map_err(|e| AppError::MetadataWrite(format!("read {}: {e}", path.display())))?;

    if orig.len() < 4 || orig[0] != 0xFF || orig[1] != 0xD8 {
        return Err(AppError::MetadataWrite(format!(
            "{}: not a JPEG (missing SOI)",
            path.display()
        )));
    }

    let payload_len = XMP_MARKER.len() + xmp_bytes.len();
    if payload_len > MAX_SEGMENT_PAYLOAD {
        return Err(AppError::MetadataWrite(format!(
            "XMP packet too large for a single JPEG APP1 segment ({} bytes)",
            payload_len
        )));
    }
    let seg_len_field = (2 + payload_len) as u16;
    let mut new_seg = Vec::with_capacity(4 + payload_len);
    new_seg.extend_from_slice(&[0xFF, 0xE1]);
    new_seg.extend_from_slice(&seg_len_field.to_be_bytes());
    new_seg.extend_from_slice(XMP_MARKER);
    new_seg.extend_from_slice(xmp_bytes);

    // Emit SOI, then our new XMP APP1 (Adobe convention says XMP should be
    // the first APP1 in the file), then copy the remaining segments while
    // dropping any pre-existing standard-XMP or ExtendedXMP APP1s.
    let mut out = Vec::with_capacity(orig.len() + new_seg.len());
    out.extend_from_slice(&orig[0..2]);
    out.extend_from_slice(&new_seg);

    let mut i = 2usize;
    while i < orig.len() {
        if orig[i] != 0xFF {
            out.extend_from_slice(&orig[i..]);
            break;
        }
        let seg_start = i;
        while i < orig.len() && orig[i] == 0xFF {
            i += 1;
        }
        if i >= orig.len() {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        let marker = orig[i];
        i += 1;

        if matches!(marker, 0xD8 | 0xD9 | 0x01) || (0xD0..=0xD7).contains(&marker) {
            out.extend_from_slice(&orig[seg_start..i]);
            continue;
        }
        // Start of Scan — compressed image data + EOI, copy verbatim.
        if marker == 0xDA {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        if i + 2 > orig.len() {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        let seg_len = u16::from_be_bytes([orig[i], orig[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > orig.len() {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        let payload_start = i + 2;
        let seg_end = i + seg_len;

        let is_std_xmp = marker == 0xE1
            && seg_end.saturating_sub(payload_start) >= XMP_MARKER.len()
            && orig[payload_start..payload_start + XMP_MARKER.len()] == *XMP_MARKER;
        let is_ext_xmp = marker == 0xE1
            && seg_end.saturating_sub(payload_start) >= XMP_EXT_MARKER.len()
            && orig[payload_start..payload_start + XMP_EXT_MARKER.len()] == *XMP_EXT_MARKER;

        if !is_std_xmp && !is_ext_xmp {
            out.extend_from_slice(&orig[seg_start..seg_end]);
        }
        i = seg_end;
    }

    common::atomic_write_bytes(path, &out)
}

// keep the compiler happy if `ExifBits` isn't referenced elsewhere via alias.
#[allow(dead_code)]
type _UnusedExifBits = ExifBits;
