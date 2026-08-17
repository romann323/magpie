pub mod metadata;
pub mod scanner;
pub mod thumbnail;

use crate::db::Db;
use std::path::PathBuf;
use std::sync::Arc;

/// Global services shared with all Tauri commands.
pub struct AppServices {
    pub db: Db,
    pub app_data_dir: PathBuf,
    pub thumb_cache_dir: PathBuf,
}

impl AppServices {
    pub fn new(app_data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let thumb_cache_dir = app_data_dir.join("thumbs");
        std::fs::create_dir_all(&thumb_cache_dir)?;
        let db_path = app_data_dir.join("picorg.db");
        let db = Db::open(&db_path)?;

        Ok(Arc::new(Self {
            db,
            app_data_dir,
            thumb_cache_dir,
        }))
    }
}

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "jif",
    "png", "webp", "gif", "bmp",
    "tif", "tiff",
    "heic", "heif",
    // RAW (metadata only in v1)
    "cr2", "cr3", "nef", "arw", "raf", "dng", "orf", "rw2", "srw",
];

pub fn is_image_ext(ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    IMAGE_EXTS.contains(&e.as_str())
}

pub fn is_processable_by_image_crate(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "jpe" | "jfif" | "jif"
        | "png" | "webp" | "gif" | "bmp"
        | "tif" | "tiff"
    )
}
