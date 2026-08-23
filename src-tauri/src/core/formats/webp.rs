//! WebP format handler. Read-only.
//!
//! Reads XMP from the `XMP ` RIFF chunk (four bytes, note the trailing
//! space) if present. WebP dimensions can come from `VP8`, `VP8L`, or
//! `VP8X` header parsing when the `image` crate can't decode the file.

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::path::Path;

const XMP_FOURCC: &[u8; 4] = b"XMP ";
const VP8X_FOURCC: &[u8; 4] = b"VP8X";
const VP8_FOURCC: &[u8; 4] = b"VP8 ";
const VP8L_FOURCC: &[u8; 4] = b"VP8L";

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

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        if let Some((w, h)) = common::read_dimensions(path) {
            tech.push("Dimensions", format!("{w} × {h} px"));
        } else if let Ok(bytes) = std::fs::read(path) {
            if let Some((w, h)) = dims_from_riff(&bytes) {
                tech.push("Dimensions", format!("{w} × {h} px"));
            }
        }
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
}

struct Chunk<'a> {
    fourcc: [u8; 4],
    data: &'a [u8],
}

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

fn u32_le3(bytes: &[u8]) -> u32 {
    (bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16)
}

fn parse_vp8_dims(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 10 {
        return None;
    }
    if data[3] != 0x9D || data[4] != 0x01 || data[5] != 0x2A {
        return None;
    }
    let w14 = u16::from_le_bytes([data[6], data[7]]) & 0x3FFF;
    let h14 = u16::from_le_bytes([data[8], data[9]]) & 0x3FFF;
    Some((w14 as u32, h14 as u32))
}

fn parse_vp8l_dims(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 5 || data[0] != 0x2F {
        return None;
    }
    let v = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let w = (v & 0x3FFF) + 1;
    let h = ((v >> 14) & 0x3FFF) + 1;
    Some((w, h))
}

fn dims_from_riff(bytes: &[u8]) -> Option<(u32, u32)> {
    let chunks = iterate_chunks(bytes)?;
    let first = chunks.first()?;
    match &first.fourcc {
        VP8X_FOURCC if first.data.len() >= 10 => Some((
            u32_le3(&first.data[4..7]) + 1,
            u32_le3(&first.data[7..10]) + 1,
        )),
        VP8_FOURCC => parse_vp8_dims(first.data),
        VP8L_FOURCC => parse_vp8l_dims(first.data),
        _ => None,
    }
}
