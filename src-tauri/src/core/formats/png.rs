//! PNG format handler. Read-only.
//!
//! Reads Adobe-standard XMP from an `iTXt` chunk keyed
//! `XML:com.adobe.xmp` on first scan so pre-tagged PNGs import cleanly.

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::path::Path;

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
        match extract_xmp(path)? {
            Some(bytes) => {
                let x = xmp_packet::parse_xmp(&bytes)?;
                Ok(xmp_packet::to_user_meta(&x))
            }
            None => Ok(UserMeta::default()),
        }
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
