//! One-shot migration from the pre-redesign central `library.db` to the
//! new two-tier layout (`registry.db` + per-folder `.magpie/library.db`).
//!
//! Trigger conditions (checked in [`migrate_legacy_central_db`]):
//! - `registry.db` does **not** exist in the app-data dir, AND
//! - `library.db` **does** exist in the app-data dir.
//!
//! Both files live in the same dir (Tauri's `app_data_dir`, e.g.
//! `%APPDATA%\com.magpie.app`).
//!
//! Behaviour:
//! 1. Open the legacy DB read-only.
//! 2. For every row in `library_folders`, materialise
//!    `<folder>/.magpie/library.db` with the new schema.
//! 3. Copy images (translating absolute `path` → folder-relative
//!    `rel_path`), tags, and image_tags into the per-folder DB.
//! 4. Rebuild FTS from the copied rows.
//! 5. Register the folder in the new `registry.db`.
//! 6. On success, rename the old file with a `.migrated-<ts>` suffix.
//!
//! Recoverable: if any step fails, `registry.db` is *not* created, the
//! legacy file is *not* renamed, and the next launch retries the
//! migration from scratch.

use crate::db::library::LibraryDb;
use crate::db::pool::LibraryPool;
use crate::error::AppResult;
use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Runs the migration if applicable. Returns `Ok(true)` if a migration
/// was performed (or attempted), `Ok(false)` if there was nothing to
/// migrate.
pub fn migrate_legacy_central_db(app_data_dir: &Path, pool: &LibraryPool) -> AppResult<bool> {
    let legacy = app_data_dir.join("library.db");
    let registry = app_data_dir.join(crate::db::registry::REGISTRY_FILE_NAME);
    if registry.exists() {
        return Ok(false);
    }
    if !legacy.exists() {
        return Ok(false);
    }

    tracing::info!(
        legacy = %legacy.display(),
        registry = %registry.display(),
        "starting legacy central-DB migration"
    );

    let legacy_conn = Connection::open_with_flags(
        &legacy,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;

    let folders = read_legacy_folders(&legacy_conn)?;
    let mut total_images = 0i64;

    for f in &folders {
        let root = PathBuf::from(&f.path);
        if !root.exists() {
            tracing::warn!(path = %root.display(), "legacy folder missing on disk; skipping migration");
            continue;
        }
        let db_path = crate::db::pool::library_db_path_for(&root);
        // Create the library DB standalone (registry hasn't heard of it
        // yet). We'll register it in the new registry after we know
        // migration succeeded for this folder.
        let lib = LibraryDb::open(&db_path, /* placeholder */ 0)?;
        let n = copy_folder(&legacy_conn, f.id, &root, &lib)?;
        total_images += n;
        tracing::info!(folder = %root.display(), images = n, "migrated folder");
    }

    // Now write the new registry. We do this last so a failure above
    // leaves the legacy DB the untouched source of truth. Rewriting
    // library.db files is idempotent because we upsert by rel_path.
    for f in &folders {
        let root = PathBuf::from(&f.path);
        if !root.exists() {
            continue;
        }
        let row = pool.add_folder(&root)?;
        // Copy last_scan_at from the legacy row.
        if let Some(ts) = f.last_scan_at {
            let _ = pool.set_last_scan_at(row.id, ts);
        }
    }

    // Rename the legacy file so it doesn't re-trigger next launch.
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let renamed = app_data_dir.join(format!("library.db.migrated-{ts}"));
    match std::fs::rename(&legacy, &renamed) {
        Ok(()) => tracing::info!(
            from = %legacy.display(),
            to = %renamed.display(),
            images = total_images,
            "legacy DB renamed after migration"
        ),
        Err(e) => tracing::warn!(error = %e, "could not rename legacy DB; will retry next launch"),
    }
    Ok(true)
}

struct LegacyFolder {
    id: i64,
    path: String,
    last_scan_at: Option<i64>,
}

fn read_legacy_folders(conn: &Connection) -> AppResult<Vec<LegacyFolder>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, last_scan_at FROM library_folders ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LegacyFolder {
            id: row.get(0)?,
            path: row.get(1)?,
            last_scan_at: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn copy_folder(
    legacy: &Connection,
    legacy_folder_id: i64,
    folder_root: &Path,
    lib: &LibraryDb,
) -> AppResult<i64> {
    let mut count = 0i64;
    // 1) images
    let mut stmt = legacy.prepare(
        "SELECT id, path, filename, ext, size_bytes, mtime_ms,
                width, height, content_hash, taken_at,
                camera_make, camera_model, title
         FROM images WHERE folder_id = ?1",
    )?;
    let images: Vec<LegacyImage> = stmt
        .query_map(params![legacy_folder_id], |row| {
            Ok(LegacyImage {
                id: row.get(0)?,
                path: row.get(1)?,
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
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut new_conn = lib.lock()?;
    let tx = new_conn.transaction()?;

    // Map legacy image id → new local id, so we can migrate image_tags.
    let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();

    for img in &images {
        let rel = match to_rel_path(folder_root, &img.path) {
            Some(r) => r,
            None => {
                tracing::warn!(
                    root = %folder_root.display(),
                    path = %img.path,
                    "legacy image path is not under its folder root; skipping"
                );
                continue;
            }
        };
        let now = chrono::Utc::now().timestamp_millis();
        tx.execute(
            "INSERT INTO images
               (rel_path, filename, ext, size_bytes, mtime_ms,
                width, height, content_hash, taken_at,
                camera_make, camera_model, title, imported_at, missing)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0)
             ON CONFLICT(rel_path) DO NOTHING",
            params![
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
            "SELECT id FROM images WHERE rel_path = ?1",
            params![rel],
            |r| r.get(0),
        )?;
        id_map.insert(img.id, new_id);
        count += 1;
    }

    // 2) tags: legacy has one central tags table; only the tags actually
    // used by *this* folder's images get carried over. We look up
    // image_tags for this folder's images and copy the referenced tag
    // names.
    let legacy_image_ids: Vec<i64> = images.iter().map(|i| i.id).collect();
    if !legacy_image_ids.is_empty() {
        let placeholders = (0..legacy_image_ids.len())
            .map(|_| "?".to_string())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT it.image_id, t.name
             FROM image_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE it.image_id IN ({placeholders})"
        );
        let args: Vec<rusqlite::types::Value> = legacy_image_ids
            .iter()
            .map(|i| rusqlite::types::Value::Integer(*i))
            .collect();
        let mut stmt = legacy.prepare(&sql)?;
        let pairs = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        for (legacy_img_id, name) in pairs {
            let Some(&new_id) = id_map.get(&legacy_img_id) else {
                continue;
            };
            tx.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![name.trim()],
            )?;
            let tag_id: i64 = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![name.trim()],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO image_tags (image_id, tag_id) VALUES (?1, ?2)",
                params![new_id, tag_id],
            )?;
        }
    }

    // 3) Rebuild FTS rows for everything we just inserted.
    for &new_id in id_map.values() {
        tx.execute(
            "DELETE FROM images_fts WHERE rowid = ?1",
            params![new_id],
        )?;
        let (filename, title): (String, Option<String>) = tx.query_row(
            "SELECT filename, title FROM images WHERE id = ?1",
            params![new_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut stmt2 = tx.prepare(
            "SELECT t.name FROM tags t
             JOIN image_tags it ON it.tag_id = t.id
             WHERE it.image_id = ?1",
        )?;
        let tag_names: Vec<String> = stmt2
            .query_map(params![new_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt2);
        let tags_joined = tag_names.join(" ");
        tx.execute(
            "INSERT INTO images_fts(rowid, title, filename, tags)
             VALUES (?1, ?2, ?3, ?4)",
            params![new_id, title.unwrap_or_default(), filename, tags_joined],
        )?;
    }

    tx.commit()?;
    Ok(count)
}

struct LegacyImage {
    id: i64,
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

/// Compute the folder-relative path of `image_path` under `folder_root`.
/// Handles case-insensitive prefix matching on Windows (paths in the
/// legacy DB are absolute, sometimes with mixed casing after Windows
/// path resolution). Returns `None` if the image isn't under the root.
fn to_rel_path(folder_root: &Path, image_path: &str) -> Option<String> {
    let img_norm = image_path.replace('/', "\\");
    let root_norm = folder_root.to_string_lossy().replace('/', "\\");
    let img_ci = img_norm.to_ascii_lowercase();
    let root_ci = root_norm.to_ascii_lowercase();
    let stripped = if img_ci.starts_with(&root_ci) {
        &img_norm[root_norm.len()..]
    } else {
        return None;
    };
    let stripped = stripped
        .trim_start_matches(['\\', '/'])
        .replace('\\', "/");
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
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
