//! WebP format handler.
//!
//! WebP is a RIFF container:
//!
//! ```text
//! "RIFF" <u32 le size> "WEBP" <chunks...>
//! chunk = <4-byte type> <u32 le size> <data> [pad byte if size odd]
//! ```
//!
//! XMP lives in a chunk whose type is exactly `XMP ` (four bytes — note the
//! trailing space). If the container is a plain "simple" WebP (VP8 chunk
//! directly after the WEBP FourCC), we promote it to the extended layout
//! (`VP8X` header + component chunks) before appending XMP, because a
//! standalone `VP8` file has no place to put chunks. The extended-form flag
//! bit for XMP (`0x04`) is toggled on so decoders know to look for the
//! chunk.
//!
//! Reference: [Google WebP Container Specification](https://developers.google.com/speed/webp/docs/riff_container)
//! (extended file format, XMP metadata section).

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::path::Path;

const XMP_FOURCC: &[u8; 4] = b"XMP ";
const VP8X_FOURCC: &[u8; 4] = b"VP8X";
const VP8_FOURCC: &[u8; 4] = b"VP8 ";
const VP8L_FOURCC: &[u8; 4] = b"VP8L";
const EXIF_FOURCC: &[u8; 4] = b"EXIF";
const ICCP_FOURCC: &[u8; 4] = b"ICCP";
const ANIM_FOURCC: &[u8; 4] = b"ANIM";
const ALPH_FOURCC: &[u8; 4] = b"ALPH";
/// VP8X flag bits (§ VP8X chunk in the container spec).
const VP8X_FLAG_XMP: u8 = 0x04;
const VP8X_FLAG_EXIF: u8 = 0x08;
const VP8X_FLAG_ICCP: u8 = 0x20;
const VP8X_FLAG_ALPHA: u8 = 0x10;
const VP8X_FLAG_ANIM: u8 = 0x02;

pub struct WebpHandler;

impl FormatHandler for WebpHandler {
    fn name(&self) -> &'static str {
        "webp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["webp"]
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
        } else if let Ok(bytes) = std::fs::read(path) {
            if let Some((w, h)) = dims_from_riff(&bytes) {
                tech.push("Dimensions", format!("{w} × {h} px"));
            }
        }
        // kamadak-exif reads WebP EXIF chunks natively.
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
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
        let existing = extract_xmp(&bytes)
            .and_then(|b| xmp_packet::parse_xmp(&b).ok())
            .unwrap_or_default();
        let merged = xmp_packet::merge_user_edits(existing, edits);
        let xmp = xmp_packet::build_xmp_packet(&merged);
        let rewritten = rewrite_with_xmp(&bytes, xmp.as_bytes())?;
        common::atomic_write_bytes(path, &rewritten)
    }
}

/// Parsed chunk: (fourcc, data-slice-without-padding).
struct Chunk<'a> {
    fourcc: [u8; 4],
    data: &'a [u8],
}

/// Walk the RIFF chunk list. Returns `None` when the file isn't a WebP.
fn iterate_chunks(bytes: &[u8]) -> Option<Vec<Chunk<'_>>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 12usize;
    while i + 8 <= bytes.len() {
        let mut fourcc = [0u8; 4];
        fourcc.copy_from_slice(&bytes[i..i + 4]);
        let size = u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]])
            as usize;
        let data_start = i + 8;
        let data_end = data_start.saturating_add(size);
        if data_end > bytes.len() {
            break;
        }
        out.push(Chunk {
            fourcc,
            data: &bytes[data_start..data_end],
        });
        // Chunks are padded to an even byte boundary.
        let padded = data_end + (size & 1);
        i = padded;
    }
    Some(out)
}

fn extract_xmp(bytes: &[u8]) -> Option<Vec<u8>> {
    let chunks = iterate_chunks(bytes)?;
    for c in chunks {
        if &c.fourcc == XMP_FOURCC {
            return Some(c.data.to_vec());
        }
    }
    None
}

/// Rewrite a WebP file so it embeds `xmp_bytes` as an `XMP ` chunk.
///
/// - "Simple" (`VP8`/`VP8L` immediately after `WEBP`) → wrapped into
///   extended form with a `VP8X` header prepended, then the VP8/VP8L data,
///   then `XMP `.
/// - "Extended" (`VP8X` first) → keep every existing chunk, replace or
///   append `XMP `, ensure the VP8X flags include `VP8X_FLAG_XMP`.
fn rewrite_with_xmp(bytes: &[u8], xmp_bytes: &[u8]) -> AppResult<Vec<u8>> {
    let chunks = iterate_chunks(bytes).ok_or_else(|| {
        AppError::MetadataWrite("not a WebP file (bad RIFF/WEBP header)".into())
    })?;

    // Decide whether this is a simple or extended container by looking at
    // the first chunk.
    let first = chunks
        .first()
        .ok_or_else(|| AppError::MetadataWrite("WebP has no chunks".into()))?;
    let is_extended = &first.fourcc == VP8X_FOURCC;

    let mut vp8x_flags: u8 = 0;
    let mut canvas_w: u32 = 0; // for constructing a VP8X if needed
    let mut canvas_h: u32 = 0;
    let mut new_chunks: Vec<Chunk> = Vec::with_capacity(chunks.len() + 1);

    if is_extended {
        // Parse the VP8X to pull existing flags + canvas dims.
        if first.data.len() < 10 {
            return Err(AppError::MetadataWrite("VP8X chunk too short".into()));
        }
        vp8x_flags = first.data[0];
        // Canvas Width Minus One (24 bit LE), then Canvas Height Minus One.
        canvas_w = u32_le3(&first.data[4..7]) + 1;
        canvas_h = u32_le3(&first.data[7..10]) + 1;

        // Copy every non-XMP chunk verbatim (we'll rebuild VP8X with fresh flags).
        for c in chunks.iter().skip(1) {
            if &c.fourcc == XMP_FOURCC {
                continue;
            }
            new_chunks.push(Chunk {
                fourcc: c.fourcc,
                data: c.data,
            });
        }
    } else {
        // Simple form. Read canvas dims from the VP8 / VP8L payload so the
        // new VP8X advertises the right size.
        match &first.fourcc {
            b"VP8 " => {
                if let Some((w, h)) = parse_vp8_dims(first.data) {
                    canvas_w = w;
                    canvas_h = h;
                }
            }
            b"VP8L" => {
                if let Some((w, h)) = parse_vp8l_dims(first.data) {
                    canvas_w = w;
                    canvas_h = h;
                }
            }
            _ => {}
        }
        // Copy the original single image chunk into the extended layout.
        new_chunks.push(Chunk {
            fourcc: first.fourcc,
            data: first.data,
        });
    }

    // Update flags: definitely XMP now; preserve EXIF / ICCP / ALPH / ANIM
    // bits by scanning new_chunks.
    vp8x_flags |= VP8X_FLAG_XMP;
    for c in &new_chunks {
        match &c.fourcc {
            EXIF_FOURCC => vp8x_flags |= VP8X_FLAG_EXIF,
            ICCP_FOURCC => vp8x_flags |= VP8X_FLAG_ICCP,
            ANIM_FOURCC => vp8x_flags |= VP8X_FLAG_ANIM,
            ALPH_FOURCC => vp8x_flags |= VP8X_FLAG_ALPHA,
            _ => {}
        }
    }

    if canvas_w == 0 || canvas_h == 0 {
        return Err(AppError::MetadataWrite(
            "could not determine WebP canvas dimensions for VP8X header".into(),
        ));
    }

    // Build the new VP8X chunk data field.
    let vp8x_data = build_vp8x_data(vp8x_flags, canvas_w, canvas_h);

    // Assemble the RIFF payload: `WEBP` FourCC + VP8X + existing chunks + XMP.
    // The RIFF header's size field counts everything AFTER the size field
    // itself (i.e. the entire payload including the FourCC).
    let mut payload = Vec::with_capacity(bytes.len() + xmp_bytes.len() + 32);
    payload.extend_from_slice(b"WEBP");
    push_chunk(&mut payload, VP8X_FOURCC, &vp8x_data);
    for c in &new_chunks {
        push_chunk(&mut payload, &c.fourcc, c.data);
    }
    push_chunk(&mut payload, XMP_FOURCC, xmp_bytes);

    let riff_size = payload.len() as u32;
    let mut out = Vec::with_capacity(payload.len() + 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

fn push_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() & 1 != 0 {
        out.push(0);
    }
}

fn build_vp8x_data(flags: u8, canvas_w: u32, canvas_h: u32) -> Vec<u8> {
    let mut d = Vec::with_capacity(10);
    d.push(flags);
    d.extend_from_slice(&[0u8; 3]); // reserved
    let w_m1 = canvas_w - 1;
    let h_m1 = canvas_h - 1;
    d.push((w_m1 & 0xFF) as u8);
    d.push(((w_m1 >> 8) & 0xFF) as u8);
    d.push(((w_m1 >> 16) & 0xFF) as u8);
    d.push((h_m1 & 0xFF) as u8);
    d.push(((h_m1 >> 8) & 0xFF) as u8);
    d.push(((h_m1 >> 16) & 0xFF) as u8);
    d
}

fn u32_le3(bytes: &[u8]) -> u32 {
    (bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16)
}

/// Extract dimensions from a `VP8 ` (lossy) chunk. Layout:
/// `3 bytes tag` + `3 bytes signature 0x9D012A` + `u16 le width14` + `u16 le height14`.
/// Widths and heights are the low 14 bits; upper 2 bits are scaling factors.
fn parse_vp8_dims(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 10 {
        return None;
    }
    // Bytes 3..6 are the sync pattern.
    if data[3] != 0x9D || data[4] != 0x01 || data[5] != 0x2A {
        return None;
    }
    let w14 = u16::from_le_bytes([data[6], data[7]]) & 0x3FFF;
    let h14 = u16::from_le_bytes([data[8], data[9]]) & 0x3FFF;
    Some((w14 as u32, h14 as u32))
}

/// Extract dimensions from a `VP8L` (lossless) chunk. Layout:
/// `0x2f` signature byte, then a 32-bit LE pack:
/// bits 0..13 → width - 1, bits 14..27 → height - 1.
fn parse_vp8l_dims(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 5 || data[0] != 0x2F {
        return None;
    }
    let v = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let w = (v & 0x3FFF) + 1;
    let h = ((v >> 14) & 0x3FFF) + 1;
    Some((w, h))
}

/// Dimensions read directly from the RIFF bytes without decoding pixels —
/// used when the `image` crate can't handle a particular WebP variant.
fn dims_from_riff(bytes: &[u8]) -> Option<(u32, u32)> {
    let chunks = iterate_chunks(bytes)?;
    let first = chunks.first()?;
    match &first.fourcc {
        VP8X_FOURCC if first.data.len() >= 10 => {
            Some((u32_le3(&first.data[4..7]) + 1, u32_le3(&first.data[7..10]) + 1))
        }
        VP8_FOURCC => parse_vp8_dims(first.data),
        VP8L_FOURCC => parse_vp8l_dims(first.data),
        _ => None,
    }
}
