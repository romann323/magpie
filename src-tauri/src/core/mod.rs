pub mod formats;
pub mod metadata;
pub mod scanner;
pub mod thumbnail;

use crate::db::Db;
use formats::FormatRegistry;
use std::path::PathBuf;
use std::sync::Arc;

/// Global services shared with every Tauri command handler.
///
/// Everything Magpie persists lives in a single SQLite file at
/// `%APPDATA%\com.magpie.app\magpie.db`, opened once at startup by
/// [`crate::db::migrate::open_or_migrate`] (which also runs a one-shot
/// import from either of the two legacy layouts).
pub struct AppServices {
    pub db: Db,
    pub app_data_dir: PathBuf,
    pub thumb_cache_dir: PathBuf,
    pub formats: Arc<FormatRegistry>,
}

impl AppServices {
    pub fn new(app_data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let thumb_cache_dir = app_data_dir.join("thumbs");
        std::fs::create_dir_all(&thumb_cache_dir)?;
        let db = crate::db::migrate::open_or_migrate(&app_data_dir)?;
        let formats = Arc::new(FormatRegistry::new());
        Ok(Arc::new(Self {
            db,
            app_data_dir,
            thumb_cache_dir,
            formats,
        }))
    }
}

/// Convenience: which extensions the `image` crate can decode. Used
/// only by the thumbnail pipeline to decide whether it should try to
/// render a preview. All *scannable* extensions come from
/// [`FormatRegistry::all_extensions`] instead.
pub fn is_processable_by_image_crate(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "jpe" | "jfif" | "jif"
        | "png" | "webp" | "gif" | "bmp"
        | "tif" | "tiff"
    )
}
