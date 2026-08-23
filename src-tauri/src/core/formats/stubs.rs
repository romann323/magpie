//! Read-only handlers for formats Magpie recognises but doesn't have a
//! dedicated Rust reader for.
//!
//! Each stub still contributes technical metadata (dimensions from the
//! `image` crate where possible, EXIF where present, plus filesize /
//! mtime / extension) so the DetailsPanel is never empty. Because the
//! DB is now the source of truth for tags, these handlers don't need
//! to write anything — the scanner reads whatever tags Windows Explorer
//! or Lightroom already embedded and imports them into the per-folder
//! library DB.

use super::common::{self, TechnicalMeta};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::AppResult;
use std::path::Path;

pub fn all_stubs() -> Vec<Box<dyn FormatHandler>> {
    vec![
        // ---------- Still image ISOBMFF (HEIF family) ----------
        Box::new(HeifHandler {
            exts: &["heic"],
        }),
        Box::new(HeifHandler {
            exts: &["heif", "hif"],
        }),
        Box::new(HeifHandler {
            exts: &["avif"],
        }),
        // ---------- Modern raster ----------
        Box::new(ExifImageHandler {
            exts: &["jxl"],
            format_name: "jxl",
        }),
        Box::new(ExifImageHandler {
            exts: &["jp2", "jpx", "j2k", "j2c"],
            format_name: "jpeg2000",
        }),
        Box::new(ExifImageHandler {
            exts: &["jxr", "wdp", "hdp"],
            format_name: "jxr",
        }),
        // ---------- Photoshop ----------
        Box::new(ExifImageHandler {
            exts: &["psd", "psb"],
            format_name: "psd",
        }),
        // ---------- Documents ----------
        Box::new(DocumentHandler {
            exts: &["pdf"],
            format_name: "pdf",
        }),
        // ---------- Video ----------
        Box::new(VideoHandler {
            exts: &["mp4", "m4v"],
            format_name: "mp4",
        }),
        Box::new(VideoHandler {
            exts: &["mov", "qt"],
            format_name: "mov",
        }),
        Box::new(VideoHandler {
            exts: &["mkv", "mka", "mks"],
            format_name: "mkv",
        }),
        Box::new(VideoHandler {
            exts: &["webm"],
            format_name: "webm",
        }),
        Box::new(VideoHandler {
            exts: &["avi"],
            format_name: "avi",
        }),
        Box::new(VideoHandler {
            exts: &["wmv", "asf"],
            format_name: "asf",
        }),
        Box::new(VideoHandler {
            exts: &["ts", "mts", "m2ts"],
            format_name: "mpegts",
        }),
        Box::new(VideoHandler {
            exts: &["3gp", "3g2", "3gpp"],
            format_name: "3gp",
        }),
        // ---------- Camera RAW ----------
        Box::new(RawHandler {
            exts: &["cr2", "cr3", "crw"],
        }),
        Box::new(RawHandler {
            exts: &["nef", "nrw"],
        }),
        Box::new(RawHandler {
            exts: &["arw", "sr2", "srf", "arq"],
        }),
        Box::new(RawHandler {
            exts: &["raf"],
        }),
        Box::new(RawHandler {
            exts: &["orf", "ori"],
        }),
        Box::new(RawHandler {
            exts: &["rw2", "rwl"],
        }),
        Box::new(RawHandler {
            exts: &["pef"],
        }),
        Box::new(RawHandler {
            exts: &["srw"],
        }),
        Box::new(RawHandler {
            exts: &["x3f"],
        }),
        // ---------- Legacy raster (discover only) ----------
        Box::new(BasicRasterHandler {
            exts: &["bmp", "dib"],
            format_name: "bmp",
        }),
        Box::new(BasicRasterHandler {
            exts: &["exr"],
            format_name: "exr",
        }),
        Box::new(BasicRasterHandler {
            exts: &["hdr"],
            format_name: "hdr",
        }),
        Box::new(BasicRasterHandler {
            exts: &["svg"],
            format_name: "svg",
        }),
    ]
}

// ---------- HEIF family (ISOBMFF-based still) ----------

pub struct HeifHandler {
    pub exts: &'static [&'static str],
}

impl FormatHandler for HeifHandler {
    fn name(&self) -> &'static str {
        "heif"
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
}

// ---------- Formats where `image` and/or kamadak-exif can help ----------

pub struct ExifImageHandler {
    pub exts: &'static [&'static str],
    pub format_name: &'static str,
}

impl FormatHandler for ExifImageHandler {
    fn name(&self) -> &'static str {
        self.format_name
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
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
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
}

// ---------- Video ----------

pub struct VideoHandler {
    pub exts: &'static [&'static str],
    pub format_name: &'static str,
}

impl FormatHandler for VideoHandler {
    fn name(&self) -> &'static str {
        self.format_name
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Video
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
}

// ---------- Document ----------

pub struct DocumentHandler {
    pub exts: &'static [&'static str],
    pub format_name: &'static str,
}

impl FormatHandler for DocumentHandler {
    fn name(&self) -> &'static str {
        self.format_name
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Document
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
}

// ---------- Camera RAW ----------

pub struct RawHandler {
    pub exts: &'static [&'static str],
}

impl FormatHandler for RawHandler {
    fn name(&self) -> &'static str {
        "raw"
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
}

// ---------- Simple raster (discover only) ----------

pub struct BasicRasterHandler {
    pub exts: &'static [&'static str],
    pub format_name: &'static str,
}

impl FormatHandler for BasicRasterHandler {
    fn name(&self) -> &'static str {
        self.format_name
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
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
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
}
