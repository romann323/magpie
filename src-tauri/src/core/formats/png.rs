//! PNG format handler.
//!
//! Metadata lives in an `iTXt` chunk with the Adobe-standard keyword
//! `XML:com.adobe.xmp`. On write we drop any pre-existing chunk with that
//! keyword and insert a fresh one immediately after `IHDR` (which the PNG
//! spec requires to come first) so viewers see the metadata before the
//! image is decoded.

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::path::Path;

/// PNG spec signature: the first 8 bytes of every PNG file.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

pub struct PngHandler;

impl FormatHandler for PngHandler {
    fn name(&self) -> &'static str {
        "png"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["png"]
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
        // Some modern PNGs (2019+) carry an eXIf chunk. `kamadak-exif`
        // handles that transparently.
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

fn extract_xmp(path: &Path) -> AppResult<Option<Vec<u8>>> {
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::MetadataRead(format!("read {}: {e}", path.display())))?;
    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Ok(None);
    }
    let mut i = PNG_SIGNATURE.len();
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let ty = &bytes[i + 4..i + 8];
        let data_start = i + 8;
        let data_end = data_start + len;
        if data_end + 4 > bytes.len() {
            break;
        }
        if ty == b"iTXt" {
            let data = &bytes[data_start..data_end];
            if let Some(nul) = data.iter().position(|&b| b == 0) {
                let keyword = &data[..nul];
                if keyword == PNG_XMP_KEYWORD && data.len() >= nul + 5 {
                    let after_kw = nul + 1;
                    // Skip compression flag (1) + compression method (1).
                    let lang_start = after_kw + 2;
                    if lang_start < data.len() {
                        if let Some(lang_nul_off) =
                            data[lang_start..].iter().position(|&b| b == 0)
                        {
                            let after_lang = lang_start + lang_nul_off + 1;
                            if let Some(trans_nul_off) =
                                data[after_lang..].iter().position(|&b| b == 0)
                            {
                                let text_start = after_lang + trans_nul_off + 1;
                                if text_start <= data.len() {
                                    return Ok(Some(data[text_start..].to_vec()));
                                }
                            }
                        }
                    }
                }
            }
        }
        if ty == b"IEND" {
            break;
        }
        i = data_end + 4;
    }
    Ok(None)
}

fn embed_xmp(path: &Path, xmp_bytes: &[u8]) -> AppResult<()> {
    let orig = std::fs::read(path)
        .map_err(|e| AppError::MetadataWrite(format!("read {}: {e}", path.display())))?;

    if orig.len() < PNG_SIGNATURE.len() || &orig[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Err(AppError::MetadataWrite(format!(
            "{}: not a PNG (bad signature)",
            path.display()
        )));
    }

    let new_chunk = build_itxt_xmp_chunk(xmp_bytes);

    let mut out: Vec<u8> = Vec::with_capacity(orig.len() + new_chunk.len());
    out.extend_from_slice(PNG_SIGNATURE);

    let mut i = PNG_SIGNATURE.len();
    let mut inserted = false;
    while i + 8 <= orig.len() {
        let len = u32::from_be_bytes([orig[i], orig[i + 1], orig[i + 2], orig[i + 3]]) as usize;
        let ty_start = i + 4;
        let data_start = i + 8;
        let data_end = data_start + len;
        let crc_end = data_end + 4;
        if crc_end > orig.len() {
            out.extend_from_slice(&orig[i..]);
            break;
        }
        let ty = &orig[ty_start..data_start];

        let is_xmp_itxt = ty == b"iTXt"
            && orig[data_start..data_end]
                .iter()
                .position(|&b| b == 0)
                .map(|nul| &orig[data_start..data_start + nul] == PNG_XMP_KEYWORD)
                .unwrap_or(false);

        if is_xmp_itxt {
            i = crc_end;
            continue;
        }

        out.extend_from_slice(&orig[i..crc_end]);

        if !inserted && ty == b"IHDR" {
            out.extend_from_slice(&new_chunk);
            inserted = true;
        }

        i = crc_end;

        if ty == b"IEND" {
            if i < orig.len() {
                out.extend_from_slice(&orig[i..]);
            }
            break;
        }
    }

    if !inserted {
        return Err(AppError::MetadataWrite(format!(
            "{}: PNG has no IHDR chunk (corrupt file)",
            path.display()
        )));
    }

    common::atomic_write_bytes(path, &out)
}

fn build_itxt_xmp_chunk(xmp_bytes: &[u8]) -> Vec<u8> {
    let mut data =
        Vec::with_capacity(PNG_XMP_KEYWORD.len() + 1 + 1 + 1 + 1 + 1 + xmp_bytes.len());
    data.extend_from_slice(PNG_XMP_KEYWORD);
    data.push(0);           // keyword terminator
    data.push(0);           // compression flag: uncompressed
    data.push(0);           // compression method: 0
    data.push(0);           // empty language tag
    data.push(0);           // empty translated keyword
    data.extend_from_slice(xmp_bytes);

    let mut chunk = Vec::with_capacity(12 + data.len());
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"iTXt");
    chunk.extend_from_slice(&data);

    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(b"iTXt");
    crc_input.extend_from_slice(&data);
    chunk.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    chunk
}

/// CRC-32 with polynomial 0xEDB88320 (reflected IEEE 802.3) — the CRC used
/// by every PNG chunk. Simple table implementation; matches every mainstream
/// CRC-32 crate.
pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for n in 0..256u32 {
            let mut c = n;
            for _ in 0..8 {
                if c & 1 != 0 {
                    c = 0xEDB88320 ^ (c >> 1);
                } else {
                    c >>= 1;
                }
            }
            t[n as usize] = c;
        }
        t
    });
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}
