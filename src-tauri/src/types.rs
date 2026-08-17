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
    pub rating: Option<i64>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDetails {
    #[serde(flatten)]
    pub summary: ImageSummary,
    pub comment: Option<String>,
    pub tags: Vec<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub meta_written_at: Option<i64>,
    pub meta_read_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFilter {
    #[serde(default)]
    pub folder_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub rating_min: Option<i64>,
    #[serde(default)]
    pub rating_max: Option<i64>,
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
    #[serde(default)]
    pub has_comment: Option<bool>,
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
    Rating,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPatch {
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub title: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub rating: Option<Option<i64>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub comment: Option<Option<String>>,
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
