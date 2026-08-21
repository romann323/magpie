//! Read-only handlers for formats Magpie recognises but doesn't yet write
//! tags to.
//!
//! Each stub still contributes technical metadata (dimensions from the
//! `image` crate where possible, EXIF where present, plus filesize / mtime /
//! extension) so the DetailsPanel is never empty. Tag writes return
//! [`common::write_not_supported_error`] with a message the UI displays
//! verbatim.
//!
//! Adding a real writer for any format below is a matter of moving the
//! handler out of this module into its own file (see the JPEG/PNG/WebP
//! handlers for prior art).

use super::common::{self, TechnicalMeta};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::AppResult;
use std::path::Path;

pub fn all_stubs() -> Vec<Box<dyn FormatHandler>> {
    vec![
        // ---------- Still image ISOBMFF (HEIF family) ----------
        Box::new(HeifHandler {
            label: "HEIC",
            exts: &["heic"],
        }),
        Box::new(HeifHandler {
            label: "HEIF",
            exts: &["heif", "hif"],
        }),
        Box::new(HeifHandler {
            label: "AVIF",
            exts: &["avif"],
        }),
        // ---------- Modern raster ----------
        Box::new(ExifImageHandler {
            label: "JPEG XL",
            exts: &["jxl"],
            format_name: "jxl",
        }),
        Box::new(ExifImageHandler {
            label: "JPEG 2000",
            exts: &["jp2", "jpx", "j2k", "j2c"],
            format_name: "jpeg2000",
        }),
        Box::new(ExifImageHandler {
            label: "JPEG XR",
            exts: &["jxr", "wdp", "hdp"],
            format_name: "jxr",
        }),
        // ---------- Photoshop ----------
        Box::new(ExifImageHandler {
            label: "Photoshop PSD",
            exts: &["psd", "psb"],
            format_name: "psd",
        }),
        // ---------- Documents ----------
        Box::new(DocumentHandler {
            label: "PDF",
            exts: &["pdf"],
            format_name: "pdf",
        }),
        // ---------- Video ----------
        Box::new(VideoHandler {
            label: "MP4",
            exts: &["mp4", "m4v"],
            format_name: "mp4",
        }),
        Box::new(VideoHandler {
            label: "QuickTime / MOV",
            exts: &["mov", "qt"],
            format_name: "mov",
        }),
        Box::new(VideoHandler {
            label: "Matroska",
            exts: &["mkv", "mka", "mks"],
            format_name: "mkv",
        }),
        Box::new(VideoHandler {
            label: "WebM",
            exts: &["webm"],
            format_name: "webm",
        }),
        Box::new(VideoHandler {
            label: "AVI",
            exts: &["avi"],
            format_name: "avi",
        }),
        Box::new(VideoHandler {
            label: "WMV / ASF",
            exts: &["wmv", "asf"],
            format_name: "asf",
        }),
        Box::new(VideoHandler {
            label: "MPEG-TS",
            exts: &["ts", "mts", "m2ts"],
            format_name: "mpegts",
        }),
        Box::new(VideoHandler {
            label: "3GP",
            exts: &["3gp", "3g2", "3gpp"],
            format_name: "3gp",
        }),
        // ---------- Camera RAW ----------
        Box::new(RawHandler {
            label: "Canon RAW",
            exts: &["cr2", "cr3", "crw"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Nikon RAW",
            exts: &["nef", "nrw"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Sony RAW",
            exts: &["arw", "sr2", "srf", "arq"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Fujifilm RAW",
            exts: &["raf"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Olympus RAW",
            exts: &["orf", "ori"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Panasonic RAW",
            exts: &["rw2", "rwl"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Pentax RAW",
            exts: &["pef"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Samsung RAW",
            exts: &["srw"],
            format_name: "raw",
        }),
        Box::new(RawHandler {
            label: "Sigma / Foveon RAW",
            exts: &["x3f"],
            format_name: "raw",
        }),
        // ---------- Legacy raster (discover only) ----------
        Box::new(BasicRasterHandler {
            label: "Bitmap",
            exts: &["bmp", "dib"],
            format_name: "bmp",
        }),
        Box::new(BasicRasterHandler {
            label: "OpenEXR",
            exts: &["exr"],
            format_name: "exr",
        }),
        Box::new(BasicRasterHandler {
            label: "Radiance HDR",
            exts: &["hdr"],
            format_name: "hdr",
        }),
        Box::new(BasicRasterHandler {
            label: "SVG",
            exts: &["svg"],
            format_name: "svg",
        }),
    ]
}

// ---------- HEIF family (ISOBMFF-based still) ----------

pub struct HeifHandler {
    pub label: &'static str,
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
    fn can_write_tags(&self) -> bool {
        false
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        // The `image` crate cannot decode HEIC/AVIF pixels out of the box on
        // Windows without the libheif system dep, but EXIF is embedded as a
        // separate box the kamadak-exif reader can locate.
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(self.label))
    }
}

// ---------- Formats where `image` and/or kamadak-exif can help ----------

pub struct ExifImageHandler {
    pub label: &'static str,
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
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(self.label))
    }
}

// ---------- Video ----------

pub struct VideoHandler {
    pub label: &'static str,
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
    fn can_write_tags(&self) -> bool {
        false
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        // Best we can do without a container parser: file basics.
        // A future PR can add the `mp4` / `matroska` crates and pull duration,
        // resolution, codec, container-level GPS, etc.
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(self.label))
    }
}

// ---------- Document ----------

pub struct DocumentHandler {
    pub label: &'static str,
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
    fn can_write_tags(&self) -> bool {
        false
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(self.label))
    }
}

// ---------- Camera RAW ----------

pub struct RawHandler {
    pub label: &'static str,
    pub exts: &'static [&'static str],
    pub format_name: &'static str,
}

impl FormatHandler for RawHandler {
    fn name(&self) -> &'static str {
        self.format_name
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }
    fn can_write_tags(&self) -> bool {
        false
    }
    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut tech = TechnicalMeta::default();
        // Most well-behaved RAWs are TIFF-shaped and kamadak-exif can dig
        // out EXIF from them.
        let bits = common::read_exif(path);
        common::append_exif_technical(&mut tech, &bits);
        common::append_file_basics(&mut tech, path);
        tech
    }
    fn read_user(&self, _path: &Path) -> AppResult<UserMeta> {
        Ok(UserMeta::default())
    }
    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(self.label))
    }
}

// ---------- Simple raster (discover only) ----------

pub struct BasicRasterHandler {
    pub label: &'static str,
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
    fn can_write_tags(&self) -> bool {
        false
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
    fn write_user(&self, _path: &Path, _edits: &UserMeta) -> AppResult<()> {
        Err(common::write_not_supported_error(self.label))
    }
}
