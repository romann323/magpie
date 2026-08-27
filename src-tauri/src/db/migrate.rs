//! One-shot migration from the two legacy on-disk layouts into
//! `magpie.db`.
//!
//! Called by [`super::open_or_migrate`] before the app connects to the
//! new DB for the first time. Two paths are supported:
//!
//! 1. **Per-folder design** (immediately previous): `registry.db` in
//!    the app-data dir plus `<folder>\.magpie\library.db` per folder.
//! 2. **Pre-redesign** (original): a single `library.db` in the
//!    app-data dir.
//!
//! Priority: if `registry.db` exists we prefer path 1 (folders may
//! have gained tags after the earlier redesign). Otherwise we fall
//! back to path 2. In either case, on success we rename each legacy
//! file with a `.migrated-<yyyymmddThhmmss>` suffix and, when the
//! source was a `.magpie` sub-folder, delete the whole `.magpie`
//! directory.
//!
//! Migration is idempotent and recoverable: `magpie.db` is created
//! fresh, populated in one transaction per folder, and only after all
//! folders succeed do we rename the source files. If we crash
//! mid-migration the untouched legacy files re-trigger it on the next
//! launch.

use crate::db::Db;
use crate::error::AppResult;
use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Open the central DB, running the appropriate migration first when
/// legacy files are found.
pub fn open_or_migrate(app_data_dir: &Path) -> AppResult<Db> {
    std::fs::create_dir_all(app_data_dir)?;
    let new_db = app_data_dir.join(super::DB_FILE_NAME);
    let registry_db = app_data_dir.join("registry.db");
    let legacy_db = app_data_dir.join("library.db");

    if new_db.exists() {
        // Already migrated (or fresh install that already ran once).
        return Db::open(&new_db);
    }

    if registry_db.exists() {
        tracing::info!("legacy per-folder layout detected; migrating into magpie.db");
        let db = Db::open(&new_db)?;
        migrate_from_registry(app_data_dir, &registry_db, &db)?;
        return Ok(db);
    }

    if legacy_db.exists() {
        tracing::info!("legacy central library.db detected; migrating into magpie.db");
        let db = Db::open(&new_db)?;
        migrate_from_pre_redesign(app_data_dir, &legacy_db, &db)?;
        return Ok(db);
    }

    // Fresh install.
    Db::open(&new_db)
}

// ---------------------------------------------------------------------
//                Path 1: per-folder .magpie/library.db
// ---------------------------------------------------------------------

fn migrate_from_registry(
    app_data_dir: &Path,
    registry_path: &Path,
    db: &Db,
) -> AppResult<()> {
    let registry = Connection::open_with_flags(
        registry_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;

    // Read the folder list out of the old registry.
    let folders: Vec<PerFolderRegistryRow> = {
        let mut stmt = registry.prepare(
            "SELECT id, path, added_at, last_scan_at, is_available
             FROM library_folders",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PerFolderRegistryRow {
                path: row.get::<_, String>(1)?,
                added_at: row.get::<_, i64>(2)?,
                last_scan_at: row.get::<_, Option<i64>>(3)?,
                is_available: row.get::<_, i64>(4)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    // Read smart collections (they live in the registry).
    let collections: Vec<(String, String, i64)> = {
        let mut stmt = registry.prepare(
            "SELECT name, filter, sort_order FROM smart_collections",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    drop(registry);

    let mut migrated_libraries: Vec<PathBuf> = Vec::new();
    let mut migrated_dot_magpie: Vec<PathBuf> = Vec::new();
    let mut total_images = 0i64;
    let mut total_folders = 0i64;

    for f in &folders {
        let root = PathBuf::from(&f.path);
        let lib_path = root.join(".magpie").join("library.db");
        if !lib_path.exists() {
            tracing::warn!(
                folder = %root.display(),
                lib = %lib_path.display(),
                "per-folder library.db missing; skipping"
            );
            continue;
        }
        match copy_per_folder_library(&lib_path, &root, f, db) {
            Ok(n) => {
                total_images += n;
                total_folders += 1;
                migrated_libraries.push(lib_path);
                migrated_dot_magpie.push(root.join(".magpie"));
                tracing::info!(
                    folder = %root.display(),
                    images = n,
                    "migrated per-folder library"
                );
            }
            Err(e) => {
                tracing::warn!(
                    folder = %root.display(),
                    error = %e,
                    "failed to migrate per-folder library; skipping"
                );
            }
        }
    }

    // Smart collections carry over verbatim.
    db.with_conn(|conn| {
        for (name, filter, sort_order) in &collections {
            conn.execute(
                "INSERT INTO smart_collections (name, filter, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![name, filter, sort_order],
            )?;
        }
        Ok(())
    })?;

    // Rename the registry file and blow away every migrated .magpie
    // sub-folder — matches the user's "delete_all" cleanup choice.
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let renamed = app_data_dir.join(format!("registry.db.migrated-{ts}"));
    if let Err(e) = std::fs::rename(registry_path, &renamed) {
        tracing::warn!(error = %e, "could not rename legacy registry.db");
    }
    for dir in migrated_dot_magpie {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!(
                dir = %dir.display(),
                error = %e,
                "could not delete .magpie sub-folder; leaving it in place"
            );
        }
    }

    tracing::info!(
        folders = total_folders,
        images = total_images,
        "per-folder → central migration complete"
    );
    Ok(())
}

struct PerFolderRegistryRow {
    path: String,
    added_at: i64,
    last_scan_at: Option<i64>,
    is_available: bool,
}

/// Copy one `<folder>\.magpie\library.db` into `magpie.db`. Returns
/// number of images imported.
fn copy_per_folder_library(
    lib_path: &Path,
    folder_root: &Path,
    reg: &PerFolderRegistryRow,
    db: &Db,
) -> AppResult<i64> {
    let src = Connection::open_with_flags(
        lib_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;

    // Everything about this folder in one shot so we can hand back the
    // src connection before opening a write transaction on the target.
    let images: Vec<PerFolderImage> = {
        let mut stmt = src.prepare(
            "SELECT id, rel_path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at,
                    camera_make, camera_model, title, imported_at, missing
             FROM images",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PerFolderImage {
                src_id: row.get(0)?,
                rel_path: row.get(1)?,
                filename: row.get(2)?,
                ext: row.get(3)?,
                size_bytes: row.get(4)?,
                mtime_ms: row.get(5)?,
                width: row.get(6)?,
                height: row.get(7)?,
                content_hash: row.get(8)?,
                taken_at: row.get(9)?,
                camera_make: row.get(10)?,
                camera_model: row.get(11)?,
                title: row.get(12)?,
                imported_at: row.get(13)?,
                missing: row.get::<_, i64>(14)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    // Pairs of (src_image_id, tag_name).
    let tag_pairs: Vec<(i64, String)> = {
        let mut stmt = src.prepare(
            "SELECT it.image_id, t.name
             FROM image_tags it
             JOIN tags t ON t.id = it.tag_id",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    drop(src);

    let count = db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO library_folders (path, added_at, last_scan_at, is_available)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO NOTHING",
            params![
                folder_root.to_string_lossy().to_string(),
                reg.added_at,
                reg.last_scan_at,
                reg.is_available as i64,
            ],
        )?;
        let new_folder_id: i64 = tx.query_row(
            "SELECT id FROM library_folders WHERE path = ?1 COLLATE NOCASE",
            params![folder_root.to_string_lossy().to_string()],
            |r| r.get(0),
        )?;

        let mut id_map: std::collections::HashMap<i64, i64> =
            std::collections::HashMap::new();
        let mut count = 0i64;
        for img in &images {
            tx.execute(
                "INSERT INTO images
                   (folder_id, rel_path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at, camera_make, camera_model,
                    title, imported_at, missing)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(folder_id, rel_path) DO NOTHING",
                params![
                    new_folder_id,
                    img.rel_path,
                    img.filename,
                    img.ext,
                    img.size_bytes,
                    img.mtime_ms,
                    img.width,
                    img.height,
                    img.content_hash,
                    img.taken_at,
                    img.camera_make,
                    img.camera_model,
                    img.title,
                    img.imported_at,
                    img.missing as i64,
                ],
            )?;
            let new_id: i64 = tx.query_row(
                "SELECT id FROM images WHERE folder_id = ?1 AND rel_path = ?2",
                params![new_folder_id, img.rel_path],
                |r| r.get(0),
            )?;
            id_map.insert(img.src_id, new_id);
            count += 1;
        }

        for (src_img_id, name) in &tag_pairs {
            let Some(&new_img_id) = id_map.get(src_img_id) else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![name],
            )?;
            let tag_id: i64 = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO image_tags (image_id, tag_id) VALUES (?1, ?2)",
                params![new_img_id, tag_id],
            )?;
        }

        // Rebuild FTS rows for the batch we just inserted.
        for &new_id in id_map.values() {
            rebuild_fts_row_tx(&tx, new_id)?;
        }
        tx.commit()?;
        Ok(count)
    })?;

    Ok(count)
}

struct PerFolderImage {
    src_id: i64,
    rel_path: String,
    filename: String,
    ext: String,
    size_bytes: i64,
    mtime_ms: i64,
    width: Option<i64>,
    height: Option<i64>,
    content_hash: Option<String>,
    taken_at: Option<i64>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    title: Option<String>,
    imported_at: i64,
    missing: bool,
}

// ---------------------------------------------------------------------
//                Path 2: pre-redesign central library.db
// ---------------------------------------------------------------------

fn migrate_from_pre_redesign(
    app_data_dir: &Path,
    legacy_path: &Path,
    db: &Db,
) -> AppResult<()> {
    let src = Connection::open_with_flags(
        legacy_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;

    let folders: Vec<LegacyFolder> = {
        let mut stmt = src.prepare(
            "SELECT id, path, added_at, last_scan_at
             FROM library_folders",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LegacyFolder {
                src_id: row.get(0)?,
                path: row.get(1)?,
                added_at: row.get(2)?,
                last_scan_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let images: Vec<LegacyImage> = {
        let mut stmt = src.prepare(
            "SELECT id, folder_id, path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at,
                    camera_make, camera_model, title
             FROM images",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LegacyImage {
                src_id: row.get(0)?,
                src_folder_id: row.get(1)?,
                path: row.get(2)?,
                filename: row.get(3)?,
                ext: row.get(4)?,
                size_bytes: row.get(5)?,
                mtime_ms: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                content_hash: row.get(9)?,
                taken_at: row.get(10)?,
                camera_make: row.get(11)?,
                camera_model: row.get(12)?,
                title: row.get(13)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let tag_pairs: Vec<(i64, String)> = {
        let mut stmt = src.prepare(
            "SELECT it.image_id, t.name
             FROM image_tags it
             JOIN tags t ON t.id = it.tag_id",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let collections: Vec<(String, String, i64)> = {
        // Older DBs might not have smart_collections at all; be forgiving.
        let has_table: bool = src
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'smart_collections'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        if has_table {
            let mut stmt = src.prepare(
                "SELECT name, filter, sort_order FROM smart_collections",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        }
    };

    drop(src);

    let (folders_migrated, images_migrated) = db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;

        // Folders first — remember mapping from legacy → new id.
        let mut folder_id_map: std::collections::HashMap<i64, i64> =
            std::collections::HashMap::new();
        let mut folders_out = 0i64;
        for f in &folders {
            tx.execute(
                "INSERT INTO library_folders (path, added_at, last_scan_at, is_available)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(path) DO NOTHING",
                params![f.path, f.added_at, f.last_scan_at],
            )?;
            let new_id: i64 = tx.query_row(
                "SELECT id FROM library_folders WHERE path = ?1 COLLATE NOCASE",
                params![f.path],
                |r| r.get(0),
            )?;
            folder_id_map.insert(f.src_id, new_id);
            folders_out += 1;
        }
        let _ = folders_out;

        // Images: convert absolute paths to folder-relative.
        let root_by_src_id: std::collections::HashMap<i64, PathBuf> = folders
            .iter()
            .map(|f| (f.src_id, PathBuf::from(&f.path)))
            .collect();

        let mut image_id_map: std::collections::HashMap<i64, i64> =
            std::collections::HashMap::new();
        let mut count = 0i64;
        for img in &images {
            let Some(root) = root_by_src_id.get(&img.src_folder_id) else {
                continue;
            };
            let Some(rel) = to_rel_path(root, &img.path) else {
                tracing::warn!(
                    root = %root.display(),
                    image = %img.path,
                    "legacy image path is not under its folder root; skipping"
                );
                continue;
            };
            let Some(&new_folder_id) = folder_id_map.get(&img.src_folder_id) else {
                continue;
            };
            let now = chrono::Utc::now().timestamp_millis();
            tx.execute(
                "INSERT INTO images
                   (folder_id, rel_path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at, camera_make, camera_model,
                    title, imported_at, missing)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0)
                 ON CONFLICT(folder_id, rel_path) DO NOTHING",
                params![
                    new_folder_id,
                    rel,
                    img.filename,
                    img.ext,
                    img.size_bytes,
                    img.mtime_ms,
                    img.width,
                    img.height,
                    img.content_hash,
                    img.taken_at,
                    img.camera_make,
                    img.camera_model,
                    img.title,
                    now,
                ],
            )?;
            let new_id: i64 = tx.query_row(
                "SELECT id FROM images WHERE folder_id = ?1 AND rel_path = ?2",
                params![new_folder_id, rel],
                |r| r.get(0),
            )?;
            image_id_map.insert(img.src_id, new_id);
            count += 1;
        }

        for (src_img_id, name) in &tag_pairs {
            let Some(&new_img_id) = image_id_map.get(src_img_id) else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![name],
            )?;
            let tag_id: i64 = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO image_tags (image_id, tag_id) VALUES (?1, ?2)",
                params![new_img_id, tag_id],
            )?;
        }

        for (name, filter, sort_order) in &collections {
            tx.execute(
                "INSERT INTO smart_collections (name, filter, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![name, filter, sort_order],
            )?;
        }

        // Rebuild FTS rows.
        for &new_id in image_id_map.values() {
            rebuild_fts_row_tx(&tx, new_id)?;
        }

        tx.commit()?;
        Ok((folder_id_map.len() as i64, count))
    })?;

    // Rename the source so it doesn't re-trigger next launch.
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let renamed = app_data_dir.join(format!("library.db.migrated-{ts}"));
    if let Err(e) = std::fs::rename(legacy_path, &renamed) {
        tracing::warn!(error = %e, "could not rename legacy library.db");
    }

    tracing::info!(
        folders = folders_migrated,
        images = images_migrated,
        "pre-redesign → central migration complete"
    );
    Ok(())
}

struct LegacyFolder {
    src_id: i64,
    path: String,
    added_at: i64,
    last_scan_at: Option<i64>,
}

struct LegacyImage {
    src_id: i64,
    src_folder_id: i64,
    path: String,
    filename: String,
    ext: String,
    size_bytes: i64,
    mtime_ms: i64,
    width: Option<i64>,
    height: Option<i64>,
    content_hash: Option<String>,
    taken_at: Option<i64>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    title: Option<String>,
}

// ---------------------------------------------------------------------
//                          Helpers
// ---------------------------------------------------------------------

/// Compute the folder-relative path of `image_path` under
/// `folder_root`. Case-insensitive prefix matching on Windows. Returns
/// `None` when the image isn't under the root.
fn to_rel_path(folder_root: &Path, image_path: &str) -> Option<String> {
    let img_norm = image_path.replace('/', "\\");
    let root_norm = folder_root.to_string_lossy().replace('/', "\\");
    let img_ci = img_norm.to_ascii_lowercase();
    let root_ci = root_norm.to_ascii_lowercase();
    if !img_ci.starts_with(&root_ci) {
        return None;
    }
    let stripped = &img_norm[root_norm.len()..];
    let stripped = stripped
        .trim_start_matches(['\\', '/'])
        .replace('\\', "/");
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

fn rebuild_fts_row_tx(tx: &rusqlite::Transaction, image_id: i64) -> AppResult<()> {
    tx.execute(
        "DELETE FROM images_fts WHERE rowid = ?1",
        params![image_id],
    )?;
    let (filename, title): (String, Option<String>) = tx.query_row(
        "SELECT filename, title FROM images WHERE id = ?1",
        params![image_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let tags: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT t.name FROM tags t
             JOIN image_tags it ON it.tag_id = t.id
             WHERE it.image_id = ?1",
        )?;
        let iter = stmt.query_map(params![image_id], |row| row.get::<_, String>(0))?;
        iter.collect::<Result<Vec<_>, _>>()?
    };
    tx.execute(
        "INSERT INTO images_fts(rowid, title, filename, tags)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            image_id,
            title.unwrap_or_default(),
            filename,
            tags.join(" ")
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod rel_path_tests {
    use super::to_rel_path;
    use std::path::Path;

    #[test]
    fn windows_forward_and_back_slashes_normalise() {
        let root = Path::new(r"C:\photos\2023");
        assert_eq!(
            to_rel_path(root, r"C:\photos\2023\sub\a.jpg").as_deref(),
            Some("sub/a.jpg")
        );
        assert_eq!(
            to_rel_path(root, "C:/photos/2023/a.jpg").as_deref(),
            Some("a.jpg")
        );
    }

    #[test]
    fn case_insensitive_prefix_on_windows() {
        let root = Path::new(r"C:\Photos\2023");
        assert_eq!(
            to_rel_path(root, r"c:\photos\2023\sub\A.JPG").as_deref(),
            Some("sub/A.JPG")
        );
    }

    #[test]
    fn returns_none_when_outside_root() {
        let root = Path::new(r"C:\photos\2023");
        assert!(to_rel_path(root, r"D:\elsewhere\a.jpg").is_none());
    }
}
