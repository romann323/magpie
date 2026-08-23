//! JPEG format handler. Read-only.
//!
//! Reads the standard XMP APP1 segment (`http://ns.adobe.com/xap/1.0/`)
//! on first scan so libraries pre-tagged with Lightroom/Bridge import
//! cleanly. Writes never touch the file — tags live in the per-folder
//! DB instead.

use super::common::{self, TechnicalMeta};
use super::xmp_packet;
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::{AppError, AppResult};
use std::io::Read;
use std::path::Path;

const XMP_MARKER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

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

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        let (w, h) = common::read_dimensions(path)
            .or_else(|| {
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
}

/// Read the JPEG file's leading header (up to 2 MiB) and hand back the
/// XMP packet bytes if an XMP APP1 segment is present.
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
