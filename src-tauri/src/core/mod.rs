pub mod formats;
pub mod metadata;
pub mod scanner;
pub mod thumbnail;

use crate::db::pool::LibraryPool;
use formats::FormatRegistry;
use std::path::PathBuf;
use std::sync::Arc;

/// Global services shared with every Tauri command handler.
///
/// After the DB redesign there's no single `Db` anymore — access to
/// stored data goes through [`LibraryPool`], which fans out to the
/// registry connection (for cross-folder search) and per-folder
/// library connections (for scanning, writes, tag edits).
pub struct AppServices {
    pub pool: Arc<LibraryPool>,
    pub app_data_dir: PathBuf,
    pub thumb_cache_dir: PathBuf,
    pub formats: Arc<FormatRegistry>,
}

impl AppServices {
    pub fn new(app_data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let thumb_cache_dir = app_data_dir.join("thumbs");
        std::fs::create_dir_all(&thumb_cache_dir)?;
        let registry_path = app_data_dir.join(crate::db::registry::REGISTRY_FILE_NAME);
        let pool = LibraryPool::open(&registry_path)?;

        // One-shot upgrade from the pre-redesign single-DB layout. Any
        // failure is logged but not fatal — the registry might just be
        // empty on first launch of the new build.
        if let Err(e) = crate::db::legacy_migration::migrate_legacy_central_db(&app_data_dir, &pool)
        {
            tracing::warn!(error = %e, "legacy DB migration failed");
        }

        let formats = Arc::new(FormatRegistry::new());
        Ok(Arc::new(Self {
            pool,
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
