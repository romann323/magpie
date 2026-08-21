//! GIF format handler (89a only — 87a has no Application Extension block).
//!
//! XMP is stored in an Application Extension block whose identifier + auth
//! code is `"XMP DataXMP"` (8 bytes + 3 bytes). Layout:
//!
//! ```text
//! 0x21 0xFF 0x0B  "XMP DataXMP"   <XMP packet bytes...>   <magic trailer>
//! ```
//!
//! The XMP data is *not* split into GIF's usual 255-byte sub-blocks; instead
//! Adobe defined a "magic trailer" of 258 bytes that acts as a self-parsing
//! sub-block header ending in a 0-length terminator. This is byte-identical
//! to Adobe's convention (see XMP Specification Part 3, § GIF89a).

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::path::Path;

const GIF89A_HEADER: &[u8] = b"GIF89a";
const GIF87A_HEADER: &[u8] = b"GIF87a";
const APP_EXT_INTRODUCER: u8 = 0x21;
const APP_EXT_LABEL: u8 = 0xFF;
const APP_ID_AUTH: &[u8; 11] = b"XMP DataXMP";
/// Adobe's magic trailer for XMP-in-GIF (0x01, 0xFF, 0xFE, ..., 0x00).
/// This is a decrementing byte sequence from 0x01 → 0x00 preceded by
/// 0x01 0xFF 0xFE. Rebuilt below.
fn xmp_magic_trailer() -> [u8; 258] {
    let mut t = [0u8; 258];
    t[0] = 0x01;
    t[1] = 0xFF;
    t[2] = 0xFE;
    // Bytes at index 3..257 count DOWN from 0xFD to 0x00.
    for (i, byte) in (0..=0xFDu8).rev().enumerate() {
        t[3 + i] = byte;
    }
    // Ensure last byte is 0x00 (block terminator).
    t[257] = 0x00;
    t
}

pub struct GifHandler;

impl FormatHandler for GifHandler {
    fn name(&self) -> &'static str {
        "gif"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gif"]
    }

    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }

    fn can_write_tags(&self) -> bool {
        true
    }

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        if let Some((w, h)) = common::read_dimensions(path) {
            tech.push("Dimensions", format!("{w} × {h} px"));
        }
        common::append_file_basics(&mut tech, path);
        tech
    }

    fn read_user(&self, path: &Path) -> AppResult<UserMeta> {
        let bytes = std::fs::read(path)
            .map_err(|e| AppError::MetadataRead(format!("read {}: {e}", path.display())))?;
        match extract_xmp(&bytes) {
            Some(xmp) => {
                let x = xmp_packet::parse_xmp(&xmp)?;
                Ok(xmp_packet::to_user_meta(&x))
            }
            None => Ok(UserMeta::default()),
        }
    }

    fn write_user(&self, path: &Path, edits: &UserMeta) -> AppResult<()> {
        let bytes = std::fs::read(path)
            .map_err(|e| AppError::MetadataWrite(format!("read {}: {e}", path.display())))?;

        // Only GIF89a has Application Extensions. 87a would need a header
        // rewrite; rare enough to reject cleanly.
        if bytes.starts_with(GIF87A_HEADER) {
            return Err(AppError::MetadataWrite(
                "GIF87a files can't carry XMP metadata — convert to GIF89a first".into(),
            ));
        }
        if !bytes.starts_with(GIF89A_HEADER) {
            return Err(AppError::MetadataWrite("not a GIF file (bad header)".into()));
        }

        let existing = extract_xmp(&bytes)
            .and_then(|b| xmp_packet::parse_xmp(&b).ok())
            .unwrap_or_default();
        let merged = xmp_packet::merge_user_edits(existing, edits);
        let xmp = xmp_packet::build_xmp_packet(&merged);
        let rewritten = rewrite_with_xmp(&bytes, xmp.as_bytes())?;
        common::atomic_write_bytes(path, &rewritten)
    }
}

/// Find the XMP Application Extension block and return its packet bytes.
fn extract_xmp(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.starts_with(GIF89A_HEADER) {
        return None;
    }
    let start = find_app_ext(bytes, APP_ID_AUTH)?;
    // start points at the 0x21 introducer byte.
    // Data begins after: 0x21 0xFF 0x0B <11-byte AppId+Auth> = start + 3 + 11
    let data_start = start + 3 + APP_ID_AUTH.len();
    // The magic trailer is the last 258 bytes of the XMP payload; strip it.
    // Scan forward for the 0x00 terminator that ends the block. Adobe's
    // magic trailer *is* the terminator, so any 0x00 we find preceded by a
    // 0x01 byte is the end of the XMP payload.
    let mut i = data_start;
    while i + 1 < bytes.len() {
        if bytes[i] == 0x01 && bytes[i + 1] == 0xFF && i > data_start + 100 {
            // Reasonable heuristic: XMP packet is longer than 100 bytes.
            // We've hit the magic trailer.
            return Some(bytes[data_start..i].to_vec());
        }
        i += 1;
    }
    // Fallback: return everything until first 0x00 block terminator.
    let end = bytes[data_start..].iter().position(|&b| b == 0x00)?;
    Some(bytes[data_start..data_start + end].to_vec())
}

/// Locate the offset of the introducer byte of the Application Extension
/// block whose 11-byte identifier matches `app_id`.
fn find_app_ext(bytes: &[u8], app_id: &[u8; 11]) -> Option<usize> {
    let mut i = 6; // skip header "GIF89a"
    while i + 14 < bytes.len() {
        if bytes[i] == APP_EXT_INTRODUCER
            && bytes[i + 1] == APP_EXT_LABEL
            && bytes[i + 2] == 0x0B
            && &bytes[i + 3..i + 14] == app_id
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Rebuild the GIF byte-for-byte, inserting the XMP Application Extension
/// block right before the trailer byte (`0x3B`).
///
/// If an XMP Application Extension already exists it is removed first so we
/// don't stack duplicates.
fn rewrite_with_xmp(bytes: &[u8], xmp_bytes: &[u8]) -> AppResult<Vec<u8>> {
    if !bytes.starts_with(GIF89A_HEADER) {
        return Err(AppError::MetadataWrite("not a GIF89a file".into()));
    }

    // Copy everything up to but not including any existing XMP block or the
    // trailer byte, then append our new XMP block, then the trailer.
    let mut without_xmp: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        // Detect an existing XMP block and skip past it.
        if bytes[i] == APP_EXT_INTRODUCER
            && i + 14 < bytes.len()
            && bytes[i + 1] == APP_EXT_LABEL
            && bytes[i + 2] == 0x0B
            && &bytes[i + 3..i + 14] == APP_ID_AUTH
        {
            // Skip up to the terminator: our XMP block always ends with the
            // magic trailer whose last byte is 0x00, so scan for a 0x00 byte
            // preceded by more than 100 bytes.
            let mut j = i + 14;
            while j < bytes.len() {
                if bytes[j] == 0x00 && j > i + 14 + 100 {
                    j += 1;
                    break;
                }
                j += 1;
            }
            i = j;
            continue;
        }
        without_xmp.push(bytes[i]);
        i += 1;
    }

    // Now build the new XMP block and splice it right before the trailer
    // byte (0x3B).
    let trailer_pos = without_xmp
        .iter()
        .rposition(|&b| b == 0x3B)
        .ok_or_else(|| AppError::MetadataWrite("GIF has no trailer byte 0x3B".into()))?;
    let (prefix, _) = without_xmp.split_at(trailer_pos);
    let mut out = Vec::with_capacity(without_xmp.len() + xmp_bytes.len() + 260);
    out.extend_from_slice(prefix);

    // Application Extension: introducer, label, block size (11), app id+auth,
    // then XMP payload directly, then magic trailer (which ends in 0x00).
    out.push(APP_EXT_INTRODUCER);
    out.push(APP_EXT_LABEL);
    out.push(0x0B);
    out.extend_from_slice(APP_ID_AUTH);
    out.extend_from_slice(xmp_bytes);
    out.extend_from_slice(&xmp_magic_trailer());

    // Final trailer byte.
    out.push(0x3B);
    Ok(out)
}
