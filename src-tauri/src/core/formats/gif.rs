//! GIF89a format handler. Read-only.
//!
//! Reads XMP from the `"XMP DataXMP"` Application Extension block if
//! present. See XMP Specification Part 3 § GIF89a for the byte layout.

use super::common::{self, TechnicalMeta};
use super::xmp_packet::{self};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::path::Path;

const GIF89A_HEADER: &[u8] = b"GIF89a";
const APP_EXT_INTRODUCER: u8 = 0x21;
const APP_EXT_LABEL: u8 = 0xFF;
const APP_ID_AUTH: &[u8; 11] = b"XMP DataXMP";

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
}

fn extract_xmp(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.starts_with(GIF89A_HEADER) {
        return None;
    }
    let start = find_app_ext(bytes, APP_ID_AUTH)?;
    let data_start = start + 3 + APP_ID_AUTH.len();
    let mut i = data_start;
    while i + 1 < bytes.len() {
        if bytes[i] == 0x01 && bytes[i + 1] == 0xFF && i > data_start + 100 {
            return Some(bytes[data_start..i].to_vec());
        }
        i += 1;
    }
    let end = bytes[data_start..].iter().position(|&b| b == 0x00)?;
    Some(bytes[data_start..data_start + end].to_vec())
}

fn find_app_ext(bytes: &[u8], app_id: &[u8; 11]) -> Option<usize> {
    let mut i = 6;
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
