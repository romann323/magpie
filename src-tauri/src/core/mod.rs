pub mod formats;
pub mod metadata;
pub mod scanner;
pub mod thumbnail;

use crate::db::Db;
use formats::FormatRegistry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Global services shared with all Tauri commands.
pub struct AppServices {
    pub db: Db,
    pub app_data_dir: PathBuf,
    pub thumb_cache_dir: PathBuf,
    pub formats: Arc<FormatRegistry>,
    /// Per-extension cache of "Windows Shell property handler accepts
    /// writes for this file type". Populated lazily on the first file of a
    /// given extension the user looks at, so subsequent DetailsPanel loads
    /// don't re-open COM stores.
    shell_write_cache: RwLock<HashMap<String, bool>>,
}

impl AppServices {
    pub fn new(app_data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let thumb_cache_dir = app_data_dir.join("thumbs");
        std::fs::create_dir_all(&thumb_cache_dir)?;
        let db_path = app_data_dir.join(crate::paths::DB_FILE_NAME);
        let db = Db::open(&db_path)?;
        let formats = Arc::new(FormatRegistry::new());

        // One-time migration: older Magpie builds canonicalised the library
        // root at add-time, which on Windows produced verbatim (`\\?\`)
        // paths in `folders.path` and `images.path`. The Windows Shell
        // property system rejects those with `E_INVALIDARG`, so tag writes
        // through the fallback would silently fail. Strip the prefix on
        // launch so existing libraries "just work" without a rescan.
        if let Err(e) = strip_verbatim_prefix_from_db(&db) {
            tracing::warn!(error = %e, "verbatim-path migration failed");
        }

        Ok(Arc::new(Self {
            db,
            app_data_dir,
            thumb_cache_dir,
            formats,
            shell_write_cache: RwLock::new(HashMap::new()),
        }))
    }

    /// Returns `true` if the Windows Shell property handler for `path`'s
    /// extension can write `System.Keywords` (and by extension `System.Title`).
    /// Cached per-extension after the first successful open.
    pub fn shell_can_write_tags(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext.is_empty() {
            return false;
        }
        if let Ok(cache) = self.shell_write_cache.read() {
            if let Some(&cached) = cache.get(&ext) {
                return cached;
            }
        }
        let ok = formats::win_shell::can_write_tags(path);
        if let Ok(mut cache) = self.shell_write_cache.write() {
            cache.entry(ext).or_insert(ok);
        }
        ok
    }
}

/// One-time migration: rewrite every `folders.path` and `images.path` that
/// still carries a verbatim `\\?\` prefix into its friendly form. Idempotent
/// — rows already lacking the prefix are `WHERE`-clause-filtered out and
/// nothing is written. Runs once at every launch; cost is `SELECT COUNT(*)`
/// against a partial-match `LIKE` when the migration is already applied.
fn strip_verbatim_prefix_from_db(db: &Db) -> anyhow::Result<()> {
    use rusqlite::params;
    db.with_conn(|conn| {
        // We use SQLite's `SUBSTR` for the rewrite so we don't have to load
        // rows into Rust for something a couple hundred bytes wide.
        //   \\?\UNC\        (8 chars, 1-indexed rest starts at 9)
        //     →  \\{rest}    (prepend the two backslashes UNC actually uses)
        //   \\?\             (4 chars, 1-indexed rest starts at 5)
        //     →  {rest}      (rest already begins with the drive letter)
        let uncs = conn.execute(
            r"UPDATE library_folders
                 SET path = '\\' || SUBSTR(path, 9)
               WHERE path LIKE '\\?\UNC\%'",
            params![],
        )?;
        let drives = conn.execute(
            r"UPDATE library_folders
                 SET path = SUBSTR(path, 5)
               WHERE path LIKE '\\?\_:\%'",
            params![],
        )?;
        let uncs_img = conn.execute(
            r"UPDATE images
                 SET path = '\\' || SUBSTR(path, 9)
               WHERE path LIKE '\\?\UNC\%'",
            params![],
        )?;
        let drives_img = conn.execute(
            r"UPDATE images
                 SET path = SUBSTR(path, 5)
               WHERE path LIKE '\\?\_:\%'",
            params![],
        )?;
        if uncs + drives + uncs_img + drives_img > 0 {
            tracing::info!(
                folders_unc = uncs,
                folders_drive = drives,
                images_unc = uncs_img,
                images_drive = drives_img,
                "stripped verbatim \\\\?\\ prefix from library paths",
            );
        }
        Ok(())
    })
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Convenience: which extensions the `image` crate can decode. Used only by
/// the thumbnail pipeline to decide whether it should try to render a
/// preview. All *scannable* extensions come from
/// [`FormatRegistry::all_extensions`] instead.
pub fn is_processable_by_image_crate(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "jpe" | "jfif" | "jif"
        | "png" | "webp" | "gif" | "bmp"
        | "tif" | "tiff"
    )
}
