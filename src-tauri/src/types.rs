use serde::{Deserialize, Serialize};

/// Custom deserializer for `Option<Option<T>>` so that:
/// - missing field  → `None`         (no change)
/// - explicit null  → `Some(None)`   (clear the field)
/// - real value     → `Some(Some(v))` (set the field)
///
/// Serde's default treats `null` as `None` for the outer Option, so without
/// this we cannot distinguish "no change" from "clear".
pub(crate) mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub last_scan_at: Option<i64>,
    pub image_count: i64,
}

/// One row in the grid. Only the fields we actually render in the grid
/// (filename, thumbnail dimensions for placeholder sizing, timestamps for
/// sorting, title for tooltip) are here. Everything else is fetched on
/// demand by `get_image`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSummary {
    pub id: i64,
    pub folder_id: i64,
    pub path: String,
    pub filename: String,
    pub ext: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size_bytes: i64,
    pub mtime_ms: i64,
    pub taken_at: Option<i64>,
    pub title: Option<String>,
    pub content_hash: Option<String>,
}

/// Full detail record served to the DetailsPanel. Includes an ordered list of
/// read-only technical metadata that each format handler contributes
/// (dimensions, capture date, camera, GPS, duration, page count, ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDetails {
    #[serde(flatten)]
    pub summary: ImageSummary,
    pub tags: Vec<String>,
    pub meta_written_at: Option<i64>,
    pub meta_read_at: Option<i64>,
    /// Ordered `[label, value]` pairs of read-only technical metadata for
    /// the "File info" section of the DetailsPanel. Ordering is determined
    /// by the format handler.
    pub technical: Vec<[String; 2]>,
    /// Format handler identity (`"jpeg"`, `"png"`, `"webp"`, ...). The UI
    /// uses this to decide whether tag edits are supported.
    pub format_handler: String,
    /// Whether the file's format handler supports writing tags back to the
    /// file. When false, the UI disables the tag editor with an explanation.
    pub can_write_tags: bool,
    /// How Magpie will actually persist title/tags for this file. Distinct
    /// from `can_write_tags` (which is just the OR of native + shell) so the
    /// UI can explain *where* the metadata goes without duplicating logic.
    pub write_mode: WriteMode,
}

/// Which write path the backend will take for a given file.
///
/// Mirrors [`WriteMode`](../../src/types.ts) on the frontend and drives the
/// hint shown under the Tags editor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WriteMode {
    /// A native Magpie format handler embeds directly (JPEG XMP, PNG iTXt,
    /// WebP, GIF89a). Round-trippable with any other XMP-aware tool.
    Native,
    /// Windows Shell property system (RAW families, MP4/MOV, HEIC, TIFF,
    /// WMV/ASF, JXL/JXR, ...). Same mechanism as Explorer's *Details* tab.
    Shell,
    /// Neither native embed nor Shell fallback works. Title and tags live
    /// in the Magpie library only.
    LibraryOnly,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFilter {
    #[serde(default)]
    pub folder_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub tags_any: Option<Vec<String>>,
    #[serde(default)]
    pub tags_all: Option<Vec<String>>,
    #[serde(default)]
    pub tags_none: Option<Vec<String>>,
    #[serde(default)]
    pub taken_after: Option<i64>,
    #[serde(default)]
    pub taken_before: Option<i64>,
    #[serde(default)]
    pub ext: Option<Vec<String>>,
    #[serde(default)]
    pub fts: Option<String>,
    #[serde(default)]
    pub has_title: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSort {
    pub by: SortBy,
    pub dir: SortDir,
}

impl Default for ImageSort {
    fn default() -> Self {
        Self { by: SortBy::TakenAt, dir: SortDir::Desc }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    TakenAt,
    Filename,
    AddedAt,
    Size,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub offset: i64,
    pub limit: i64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { offset: 0, limit: 200 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

/// Patch supplied by the UI for a metadata edit. Title uses double-option so
/// the caller can distinguish "leave the current title alone" from "clear
/// the title". Tags fields are simple lists — set/add/remove semantics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPatch {
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub title: Option<Option<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub tags_add: Option<Vec<String>>,
    #[serde(default)]
    pub tags_remove: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub deleted: Vec<i64>,
    pub failed: Vec<DeleteFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFailure {
    pub id: i64,
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagStats {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub folder_id: i64,
    pub processed: i64,
    pub total: i64,
    pub current_path: Option<String>,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub folder_id: i64,
    pub added: i64,
    pub updated: i64,
    pub removed: i64,
    pub errors: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCollection {
    pub id: i64,
    pub name: String,
    pub filter: ImageFilter,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThumbSize {
    Small,
    Medium,
    Large,
}

impl ThumbSize {
    pub fn pixels(&self) -> u32 {
        match self {
            ThumbSize::Small => 160,
            ThumbSize::Medium => 320,
            ThumbSize::Large => 640,
        }
    }
}
