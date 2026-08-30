//! Every SQL statement Magpie runs against `magpie.db`.
//!
//! Grouped by concern:
//! - **Folders** — add / remove / list / update availability.
//! - **Images** — scan-time upsert, meta from-file import, delete,
//!   `MetadataPatch` (title / tags CRUD).
//! - **Tags** — global rename & delete.
//! - **Search** — `query_images` + `list_all_tags` (single-file SQL,
//!   no more ATTACH / UNION juggling).
//! - **Smart collections** — CRUD.
//!
//! Everything that mutates goes through a transaction that also
//! rebuilds the affected FTS row(s). Callers hold the `Db` mutex the
//! whole time via `Db::with_conn_mut`.

use crate::error::{AppError, AppResult};
use crate::types::*;
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------
//                            Folders
// ---------------------------------------------------------------------

/// Full DB shape of a `library_folders` row. IPC layer maps this into
/// [`LibraryFolder`] which also carries `imageCount`.
#[derive(Debug, Clone)]
pub struct LibraryFolderRow {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub last_scan_at: Option<i64>,
    pub is_available: bool,
}

pub fn insert_folder(conn: &Connection, path: &Path) -> AppResult<LibraryFolderRow> {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canon.to_string_lossy().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO library_folders (path, added_at, is_available)
         VALUES (?1, ?2, 1)
         ON CONFLICT(path) DO NOTHING",
        params![path_str, now],
    )?;
    get_folder_by_path(conn, &path_str)?
        .ok_or_else(|| AppError::Internal("failed to insert library folder".into()))
}

pub fn delete_folder(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute("DELETE FROM library_folders WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::FolderNotFound(id));
    }
    Ok(())
}

pub fn list_folders(conn: &Connection) -> AppResult<Vec<LibraryFolderRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, added_at, last_scan_at, is_available
         FROM library_folders ORDER BY added_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_folder)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_folder(conn: &Connection, id: i64) -> AppResult<LibraryFolderRow> {
    conn.query_row(
        "SELECT id, path, added_at, last_scan_at, is_available
         FROM library_folders WHERE id = ?1",
        params![id],
        row_to_folder,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::FolderNotFound(id),
        other => other.into(),
    })
}

pub fn get_folder_by_path(
    conn: &Connection,
    path: &str,
) -> AppResult<Option<LibraryFolderRow>> {
    Ok(conn
        .query_row(
            "SELECT id, path, added_at, last_scan_at, is_available
             FROM library_folders WHERE path = ?1 COLLATE NOCASE",
            params![path],
            row_to_folder,
        )
        .optional()?)
}

pub fn set_last_scan_at(conn: &Connection, folder_id: i64, ts: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE library_folders SET last_scan_at = ?1 WHERE id = ?2",
        params![ts, folder_id],
    )?;
    Ok(())
}

pub fn set_folder_availability(
    conn: &Connection,
    folder_id: i64,
    available: bool,
) -> AppResult<()> {
    conn.execute(
        "UPDATE library_folders SET is_available = ?1 WHERE id = ?2",
        params![available as i64, folder_id],
    )?;
    Ok(())
}

fn row_to_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryFolderRow> {
    Ok(LibraryFolderRow {
        id: row.get(0)?,
        path: row.get(1)?,
        added_at: row.get(2)?,
        last_scan_at: row.get(3)?,
        is_available: row.get::<_, i64>(4)? != 0,
    })
}

pub fn count_images_in_folder(conn: &Connection, folder_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM images WHERE folder_id = ?1 AND missing = 0",
        params![folder_id],
        |r| r.get(0),
    )?)
}

// ---------------------------------------------------------------------
//                             Images
// ---------------------------------------------------------------------

/// Per-scan file identity that [`upsert_image`] compares against.
#[derive(Debug, Clone)]
pub struct FileStat<'a> {
    pub folder_id: i64,
    pub rel_path: String,
    pub filename: &'a str,
    pub ext: &'a str,
    pub size_bytes: i64,
    pub mtime_ms: i64,
}

pub enum UpsertOutcome {
    Added { id: i64 },
    Updated { id: i64 },
    Unchanged { id: i64 },
}

impl UpsertOutcome {
    pub fn id(&self) -> i64 {
        match self {
            UpsertOutcome::Added { id }
            | UpsertOutcome::Updated { id }
            | UpsertOutcome::Unchanged { id } => *id,
        }
    }
}

pub fn upsert_image(conn: &Connection, s: &FileStat<'_>) -> AppResult<UpsertOutcome> {
    let existing: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, size_bytes, mtime_ms FROM images
             WHERE folder_id = ?1 AND rel_path = ?2",
            params![s.folder_id, s.rel_path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    match existing {
        Some((id, sz, mt)) => {
            if sz == s.size_bytes && mt == s.mtime_ms {
                conn.execute("UPDATE images SET missing = 0 WHERE id = ?1", params![id])?;
                Ok(UpsertOutcome::Unchanged { id })
            } else {
                conn.execute(
                    "UPDATE images
                       SET filename = ?1, ext = ?2, size_bytes = ?3, mtime_ms = ?4, missing = 0
                     WHERE id = ?5",
                    params![s.filename, s.ext, s.size_bytes, s.mtime_ms, id],
                )?;
                Ok(UpsertOutcome::Updated { id })
            }
        }
        None => {
            conn.execute(
                "INSERT INTO images
                   (folder_id, rel_path, filename, ext, size_bytes, mtime_ms, imported_at, missing)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                params![
                    s.folder_id,
                    s.rel_path,
                    s.filename,
                    s.ext,
                    s.size_bytes,
                    s.mtime_ms,
                    chrono::Utc::now().timestamp_millis(),
                ],
            )?;
            Ok(UpsertOutcome::Added {
                id: conn.last_insert_rowid(),
            })
        }
    }
}

/// Subset of on-disk metadata the DB indexes. Written by the scanner
/// (and the mtime-based re-read in `commands::images::get_image`).
#[derive(Default, Debug, Clone)]
pub struct ImageMetaFromFile {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub taken_at: Option<i64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

/// Applied by the scanner (and the mtime-based re-read in
/// `commands::images::get_image`). Never destroys existing tags:
///
/// - Title / dimensions / camera fields overwrite as usual.
/// - Tags read from the file's own metadata (XMP subjects, Windows
///   Shell keywords, sidecar XMP) are inserted with `source = 'auto'`
///   **only when the image doesn't already carry that name in
///   either source**. Auto tags the file no longer mentions stay put;
///   user tags are never touched here.
pub fn set_image_meta(
    conn: &mut Connection,
    image_id: i64,
    m: &ImageMetaFromFile,
) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE images SET
            width        = COALESCE(?1, width),
            height       = COALESCE(?2, height),
            taken_at     = COALESCE(?3, taken_at),
            camera_make  = COALESCE(?4, camera_make),
            camera_model = COALESCE(?5, camera_model),
            title        = ?6
         WHERE id = ?7",
        params![
            m.width,
            m.height,
            m.taken_at,
            m.camera_make,
            m.camera_model,
            m.title,
            image_id,
        ],
    )?;
    add_auto_tags_if_absent_tx(&tx, image_id, &m.tags)?;
    rebuild_fts_row_tx(&tx, image_id)?;
    tx.commit()?;
    Ok(())
}

pub fn set_content_hash(conn: &Connection, image_id: i64, hash: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE images SET content_hash = ?1 WHERE id = ?2",
        params![hash, image_id],
    )?;
    Ok(())
}

/// Mark every image in a folder that wasn't touched during the current
/// scan as `missing = 1`. Returns the number of newly-missing rows.
pub fn mark_missing_by_seen(
    conn: &mut Connection,
    folder_id: i64,
    seen_rel_paths: &std::collections::HashSet<String>,
) -> AppResult<i64> {
    let tx = conn.transaction()?;
    let mut removed = 0i64;
    let rows: Vec<(i64, String)> = {
        let mut stmt = tx.prepare(
            "SELECT id, rel_path FROM images
             WHERE folder_id = ?1 AND missing = 0",
        )?;
        let iter = stmt.query_map(params![folder_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        iter.collect::<Result<Vec<_>, _>>()?
    };
    for (id, p) in rows {
        if !seen_rel_paths.contains(&p) {
            tx.execute("UPDATE images SET missing = 1 WHERE id = ?1", params![id])?;
            removed += 1;
        }
    }
    tx.commit()?;
    Ok(removed)
}

/// Full row for the DetailsPanel. `rel_path` is folder-relative; join
/// with `library_folders.path` to build an absolute path.
///
/// Tags are split by provenance: `user_tags` are what the user typed
/// inside Magpie; `auto_tags` are what the scanner read from the file
/// itself (XMP / Windows Shell / sidecar). The same name can appear
/// in both vectors if both sources carry it.
pub struct ImageRow {
    pub id: i64,
    pub folder_id: i64,
    pub rel_path: String,
    pub filename: String,
    pub ext: String,
    pub size_bytes: i64,
    pub mtime_ms: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub content_hash: Option<String>,
    pub taken_at: Option<i64>,
    pub title: Option<String>,
    pub imported_at: i64,
    pub user_tags: Vec<String>,
    pub auto_tags: Vec<String>,
}

pub fn get_image_row(conn: &Connection, id: i64) -> AppResult<Option<ImageRow>> {
    let base: Option<ImageRow> = conn
        .query_row(
            "SELECT id, folder_id, rel_path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at, title, imported_at
             FROM images WHERE id = ?1",
            params![id],
            |row| {
                Ok(ImageRow {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    rel_path: row.get(2)?,
                    filename: row.get(3)?,
                    ext: row.get(4)?,
                    size_bytes: row.get(5)?,
                    mtime_ms: row.get(6)?,
                    width: row.get(7)?,
                    height: row.get(8)?,
                    content_hash: row.get(9)?,
                    taken_at: row.get(10)?,
                    title: row.get(11)?,
                    imported_at: row.get(12)?,
                    user_tags: Vec::new(),
                    auto_tags: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(mut row) = base else {
        return Ok(None);
    };
    row.user_tags = user_tags_for_image(conn, id)?;
    row.auto_tags = auto_tags_for_image(conn, id)?;
    Ok(Some(row))
}

/// Convenience: look up an image and eagerly resolve its absolute path
/// by joining with `library_folders`.
pub fn get_image_with_root(
    conn: &Connection,
    id: i64,
) -> AppResult<Option<(ImageRow, PathBuf)>> {
    let Some(row) = get_image_row(conn, id)? else {
        return Ok(None);
    };
    let folder = get_folder(conn, row.folder_id)?;
    Ok(Some((row, PathBuf::from(&folder.path))))
}

/// Distinct list of tag names attached to `image_id` from **either**
/// source. Used to refresh the FTS row and by tests / diagnostics that
/// want the flat vocabulary a user would see in the sidebar.
pub fn tags_for_image(conn: &Connection, image_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT t.name FROM tags t
         JOIN image_tags it ON it.tag_id = t.id
         WHERE it.image_id = ?1
         ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![image_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Tags the user typed inside Magpie (`image_tags.source = 'user'`).
pub fn user_tags_for_image(conn: &Connection, image_id: i64) -> AppResult<Vec<String>> {
    tags_for_image_by_source(conn, image_id, "user")
}

/// Tags the scanner read from the file itself (`source = 'auto'`).
pub fn auto_tags_for_image(conn: &Connection, image_id: i64) -> AppResult<Vec<String>> {
    tags_for_image_by_source(conn, image_id, "auto")
}

fn tags_for_image_by_source(
    conn: &Connection,
    image_id: i64,
    source: &str,
) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN image_tags it ON it.tag_id = t.id
         WHERE it.image_id = ?1 AND it.source = ?2
         ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![image_id, source], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Rename an image in the DB after the caller has already renamed the
/// file on disk. Updates `filename`, `rel_path` (same parent), and
/// `ext`, then rebuilds the FTS row so search finds the new name.
pub fn rename_image_row(
    conn: &mut Connection,
    image_id: i64,
    new_filename: &str,
) -> AppResult<()> {
    let (folder_id, old_rel): (i64, String) = conn.query_row(
        "SELECT folder_id, rel_path FROM images WHERE id = ?1",
        params![image_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (parent, _) = split_parent(&old_rel);
    let new_rel = if parent.is_empty() {
        new_filename.to_string()
    } else {
        format!("{parent}/{new_filename}")
    };
    let new_ext = std::path::Path::new(new_filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let tx = conn.transaction()?;
    // Guard against colliding with another row already using the target rel_path.
    let clash: Option<i64> = tx
        .query_row(
            "SELECT id FROM images
             WHERE folder_id = ?1 AND rel_path = ?2 AND id <> ?3",
            params![folder_id, new_rel, image_id],
            |r| r.get(0),
        )
        .optional()?;
    if clash.is_some() {
        return Err(AppError::BadInput(format!(
            "another image is already registered as \"{new_rel}\""
        )));
    }
    tx.execute(
        "UPDATE images SET filename = ?1, rel_path = ?2, ext = ?3 WHERE id = ?4",
        params![new_filename, new_rel, new_ext, image_id],
    )?;
    rebuild_fts_row_tx(&tx, image_id)?;
    tx.commit()?;
    Ok(())
}

/// `("sub/dir", "file.jpg")` for `"sub/dir/file.jpg"`; `("", "file.jpg")`
/// when no parent. Always uses `/` as the separator (matches the on-disk
/// storage convention).
fn split_parent(rel: &str) -> (String, String) {
    let norm = rel.replace('\\', "/");
    match norm.rsplit_once('/') {
        Some((p, f)) => (p.to_string(), f.to_string()),
        None => (String::new(), norm),
    }
}

pub fn delete_images(conn: &mut Connection, ids: &[i64]) -> AppResult<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let tx = conn.transaction()?;
    let mut removed = 0;
    for id in ids {
        removed += tx.execute("DELETE FROM images WHERE id = ?1", params![id])?;
        let _ = tx.execute("DELETE FROM images_fts WHERE rowid = ?1", params![id]);
    }
    tx.commit()?;
    Ok(removed)
}

/// Given a batch of image IDs, return `(id, folder_id, rel_path)` for
/// each. Missing rows are silently dropped.
pub fn get_paths(
    conn: &Connection,
    ids: &[i64],
) -> AppResult<Vec<(i64, i64, String)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (0..ids.len())
        .map(|_| "?".to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, folder_id, rel_path FROM images WHERE id IN ({})",
        placeholders
    );
    let args: Vec<Value> = ids.iter().map(|i| Value::Integer(*i)).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------------
//                     Automatic AI tagging
// ---------------------------------------------------------------------

/// One `(image_id, ext, fingerprint, ai_tag_hash)` row per non-missing
/// image in a folder. `fingerprint` is the value the AI pipeline
/// compares against `ai_tag_hash` to decide whether to skip.
#[derive(Debug, Clone)]
pub struct AutoTagCandidate {
    pub id: i64,
    pub rel_path: String,
    pub ext: String,
    /// `content_hash` when the scanner has computed one; falls back to
    /// `mtime_ms.to_string()` so we always have something stable.
    pub fingerprint: String,
    pub ai_tag_hash: Option<String>,
}

/// List every non-missing image in `folder_id`, along with its current
/// AI-tag fingerprint. Ordered by id so progress reporting is stable.
pub fn list_auto_tag_candidates(
    conn: &Connection,
    folder_id: i64,
) -> AppResult<Vec<AutoTagCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, ext, content_hash, mtime_ms, ai_tag_hash
         FROM images
         WHERE folder_id = ?1 AND missing = 0
         ORDER BY id",
    )?;
    let rows = stmt.query_map(params![folder_id], |row| {
        let id: i64 = row.get(0)?;
        let rel_path: String = row.get(1)?;
        let ext: String = row.get(2)?;
        let content_hash: Option<String> = row.get(3)?;
        let mtime_ms: i64 = row.get(4)?;
        let ai_tag_hash: Option<String> = row.get(5)?;
        let fingerprint = content_hash.unwrap_or_else(|| mtime_ms.to_string());
        Ok(AutoTagCandidate {
            id,
            rel_path,
            ext,
            fingerprint,
            ai_tag_hash,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Record that AI tagging just ran against `image_id` with the given
/// fingerprint. `ts_ms` is the current time in ms since epoch.
pub fn mark_image_ai_tagged(
    conn: &Connection,
    image_id: i64,
    fingerprint: &str,
    ts_ms: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE images SET ai_tagged_at = ?1, ai_tag_hash = ?2 WHERE id = ?3",
        params![ts_ms, fingerprint, image_id],
    )?;
    Ok(())
}

/// Attach `names` to `image_id` as `'auto'`-source tags. Skips names
/// the image already carries (from either source) so the row count
/// stays sane on rerun. Rebuilds the FTS row inside the same
/// transaction. Called by the automatic-AI-tagging pipeline
/// ([`crate::core::auto_tag::tag_folder`]) so its output lives in
/// the same read-only "Automatic tags" bucket as XMP/Shell-imported
/// tags rather than in the user-editable list.
pub fn add_auto_tags_for_image(
    conn: &mut Connection,
    image_id: i64,
    names: &[String],
) -> AppResult<()> {
    let tx = conn.transaction()?;
    add_auto_tags_if_absent_tx(&tx, image_id, names)?;
    rebuild_fts_row_tx(&tx, image_id)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------
//                         MetadataPatch
// ---------------------------------------------------------------------

/// UI-driven patch. Every field is optional; every tag field targets
/// the `'user'` source (auto tags are read-only from the UI).
pub struct MetadataPatch {
    /// `None`  = no change.
    /// `Some(None)`     = clear title.
    /// `Some(Some(s))`  = set title.
    pub title: Option<Option<String>>,
    /// Replace every user tag on the image with this list.
    pub tags: Option<Vec<String>>,
    /// Attach each name as a user tag (no-op when already present as
    /// a user tag; an auto row with the same name is unaffected).
    pub tags_add: Option<Vec<String>>,
    /// Remove each name from the image's user tags. An auto row with
    /// the same name (if any) stays put.
    pub tags_remove: Option<Vec<String>>,
}

/// Apply a UI-supplied patch to an image. All tag operations target
/// the `'user'` source; automatic tags (from the file itself) are
/// never added or removed here. See [`set_image_meta`] for the scanner
/// path.
pub fn apply_metadata_patch(
    conn: &mut Connection,
    image_id: i64,
    patch: &MetadataPatch,
) -> AppResult<()> {
    let tx = conn.transaction()?;
    if let Some(title) = &patch.title {
        tx.execute(
            "UPDATE images SET title = ?1 WHERE id = ?2",
            params![title, image_id],
        )?;
    }
    if let Some(tags) = &patch.tags {
        replace_user_tags_tx(&tx, image_id, tags)?;
    }
    if let Some(add) = &patch.tags_add {
        for t in add {
            let name = t.trim();
            if !name.is_empty() {
                add_tag_tx(&tx, image_id, name, "user")?;
            }
        }
    }
    if let Some(rm) = &patch.tags_remove {
        for t in rm {
            remove_user_tag_tx(&tx, image_id, t.trim())?;
        }
    }
    rebuild_fts_row_tx(&tx, image_id)?;
    tx.commit()?;
    Ok(())
}

fn tag_id_for_name_tx(tx: &rusqlite::Transaction, name: &str) -> AppResult<i64> {
    tx.execute(
        "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
        params![name],
    )?;
    Ok(tx.query_row(
        "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
        params![name],
        |r| r.get(0),
    )?)
}

/// Attach `name` to `image_id` with the given source. No-op when the
/// exact `(image, tag, source)` triple already exists.
fn add_tag_tx(
    tx: &rusqlite::Transaction,
    image_id: i64,
    name: &str,
    source: &str,
) -> AppResult<()> {
    let tag_id = tag_id_for_name_tx(tx, name)?;
    tx.execute(
        "INSERT OR IGNORE INTO image_tags (image_id, tag_id, source)
         VALUES (?1, ?2, ?3)",
        params![image_id, tag_id, source],
    )?;
    Ok(())
}

/// Remove `name` from `image_id`'s **user** tags. Auto rows with the
/// same name are left alone (they came from the file itself).
fn remove_user_tag_tx(
    tx: &rusqlite::Transaction,
    image_id: i64,
    name: &str,
) -> AppResult<()> {
    if name.is_empty() {
        return Ok(());
    }
    tx.execute(
        "DELETE FROM image_tags
         WHERE image_id = ?1
           AND source   = 'user'
           AND tag_id   = (SELECT id FROM tags WHERE name = ?2 COLLATE NOCASE)",
        params![image_id, name],
    )?;
    Ok(())
}

/// Wipe every `'user'` row for `image_id` and re-insert `tags` as
/// user rows. Auto rows are untouched.
fn replace_user_tags_tx(
    tx: &rusqlite::Transaction,
    image_id: i64,
    tags: &[String],
) -> AppResult<()> {
    tx.execute(
        "DELETE FROM image_tags WHERE image_id = ?1 AND source = 'user'",
        params![image_id],
    )?;
    for t in tags {
        let name = t.trim();
        if name.is_empty() {
            continue;
        }
        add_tag_tx(tx, image_id, name, "user")?;
    }
    Ok(())
}

/// Called by [`set_image_meta`] on every scan. For each name the file
/// currently reports, adds an `'auto'` row **only if** the image
/// doesn't already carry that name (in either source). Never deletes
/// anything, so auto tags removed from the file stay in the DB and
/// user edits are always preserved.
fn add_auto_tags_if_absent_tx(
    tx: &rusqlite::Transaction,
    image_id: i64,
    tags: &[String],
) -> AppResult<()> {
    for t in tags {
        let name = t.trim();
        if name.is_empty() {
            continue;
        }
        let already: bool = tx
            .query_row(
                "SELECT 1 FROM image_tags it
                 JOIN tags t ON t.id = it.tag_id
                 WHERE it.image_id = ?1 AND t.name = ?2 COLLATE NOCASE
                 LIMIT 1",
                params![image_id, name],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if already {
            continue;
        }
        add_tag_tx(tx, image_id, name, "auto")?;
    }
    Ok(())
}

fn rebuild_fts_row_tx(tx: &rusqlite::Transaction, image_id: i64) -> AppResult<()> {
    tx.execute(
        "DELETE FROM images_fts WHERE rowid = ?1",
        params![image_id],
    )?;
    let row: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT filename, title FROM images WHERE id = ?1",
            params![image_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((filename, title)) = row {
        // DISTINCT so an image with the same tag from both sources
        // doesn't get the token indexed twice.
        let tags: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT t.name FROM tags t
                 JOIN image_tags it ON it.tag_id = t.id
                 WHERE it.image_id = ?1",
            )?;
            let iter = stmt.query_map(params![image_id], |row| row.get::<_, String>(0))?;
            iter.collect::<Result<Vec<_>, _>>()?
        };
        let joined = tags.join(" ");
        tx.execute(
            "INSERT INTO images_fts(rowid, title, filename, tags)
             VALUES (?1, ?2, ?3, ?4)",
            params![image_id, title.unwrap_or_default(), filename, joined],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
//                           Tag mutations
// ---------------------------------------------------------------------

pub fn rename_tag(conn: &mut Connection, old: &str, new: &str) -> AppResult<()> {
    let new = new.trim();
    if new.is_empty() {
        return Err(AppError::BadInput("new tag name is empty".into()));
    }
    let old = old.trim();
    let tx = conn.transaction()?;
    let updated = tx.execute(
        "UPDATE OR IGNORE tags SET name = ?1 WHERE name = ?2 COLLATE NOCASE",
        params![new, old],
    )?;
    if updated == 0 {
        // Merge: `new` already exists.
        let old_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![old],
                |r| r.get(0),
            )
            .optional()?;
        let new_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![new],
                |r| r.get(0),
            )
            .optional()?;
        if let (Some(o), Some(n)) = (old_id, new_id) {
            if o != n {
                // Preserve provenance while merging: each (image, source)
                // that pointed at `o` should now point at `n`; the new
                // composite PK deduplicates automatically.
                tx.execute(
                    "INSERT OR IGNORE INTO image_tags (image_id, tag_id, source)
                     SELECT image_id, ?2, source FROM image_tags WHERE tag_id = ?1",
                    params![o, n],
                )?;
                tx.execute("DELETE FROM tags WHERE id = ?1", params![o])?;
            }
        }
    }
    // FTS rows still index the old tag name; rebuild for everyone that
    // used to have it.
    let affected: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT it.image_id FROM image_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE t.name = ?1 COLLATE NOCASE",
        )?;
        let iter = stmt.query_map(params![new], |r| r.get::<_, i64>(0))?;
        iter.collect::<Result<Vec<_>, _>>()?
    };
    for id in affected {
        rebuild_fts_row_tx(&tx, id)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn delete_tag(conn: &mut Connection, name: &str) -> AppResult<()> {
    let name = name.trim();
    let tx = conn.transaction()?;
    let affected: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT it.image_id FROM image_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE t.name = ?1 COLLATE NOCASE",
        )?;
        let iter = stmt.query_map(params![name], |r| r.get::<_, i64>(0))?;
        iter.collect::<Result<Vec<_>, _>>()?
    };
    tx.execute(
        "DELETE FROM tags WHERE name = ?1 COLLATE NOCASE",
        params![name],
    )?;
    for id in affected {
        rebuild_fts_row_tx(&tx, id)?;
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------
//                              Search
// ---------------------------------------------------------------------

pub fn list_all_tags(
    conn: &Connection,
    prefix: Option<&str>,
) -> AppResult<Vec<TagStats>> {
    let prefix_pat = prefix
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    // COUNT(DISTINCT it.image_id): an image with the same tag as
    // both 'auto' and 'user' is one image, not two, in the sidebar.
    let (sql, args): (&str, Vec<Value>) = if let Some(p) = &prefix_pat {
        (
            "SELECT t.name, COUNT(DISTINCT it.image_id) AS c
             FROM tags t
             LEFT JOIN image_tags it ON it.tag_id = t.id
             WHERE t.name LIKE ? COLLATE NOCASE
             GROUP BY t.id
             ORDER BY c DESC, t.name COLLATE NOCASE
             LIMIT 500",
            vec![Value::Text(format!("{}%", p))],
        )
    } else {
        (
            "SELECT t.name, COUNT(DISTINCT it.image_id) AS c
             FROM tags t
             LEFT JOIN image_tags it ON it.tag_id = t.id
             GROUP BY t.id
             ORDER BY c DESC, t.name COLLATE NOCASE
             LIMIT 500",
            Vec::new(),
        )
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
        Ok(TagStats {
            name: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn query_images(
    conn: &Connection,
    filter: &ImageFilter,
    sort: &ImageSort,
    page: &Pagination,
) -> AppResult<Page<ImageSummary>> {
    // Folder roots for absolute-path stitching.
    let folder_paths: std::collections::HashMap<i64, String> = {
        let mut stmt = conn.prepare("SELECT id, path FROM library_folders")?;
        let iter = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        iter.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect()
    };

    let (where_sql, args) = build_where(filter);
    let order_by = match (sort.by, sort.dir) {
        (SortBy::TakenAt, SortDir::Asc) => "COALESCE(taken_at, mtime_ms) ASC, id ASC",
        (SortBy::TakenAt, SortDir::Desc) => "COALESCE(taken_at, mtime_ms) DESC, id DESC",
        (SortBy::Filename, SortDir::Asc) => "filename COLLATE NOCASE ASC, id ASC",
        (SortBy::Filename, SortDir::Desc) => "filename COLLATE NOCASE DESC, id DESC",
        (SortBy::AddedAt, SortDir::Asc) => "id ASC",
        (SortBy::AddedAt, SortDir::Desc) => "id DESC",
        (SortBy::Size, SortDir::Asc) => "size_bytes ASC, id ASC",
        (SortBy::Size, SortDir::Desc) => "size_bytes DESC, id DESC",
    };
    let count_sql = format!("SELECT COUNT(*) FROM images WHERE {where_sql}");
    let sql = format!(
        "SELECT id, folder_id, rel_path, filename, ext, size_bytes, mtime_ms,
                width, height, content_hash, taken_at, title
         FROM images
         WHERE {where_sql}
         ORDER BY {order_by}
         LIMIT ? OFFSET ?"
    );

    let total: i64 =
        conn.query_row(&count_sql, params_from_iter(args.iter()), |r| r.get(0))?;

    let mut paged_args = args.clone();
    paged_args.push(Value::Integer(page.limit));
    paged_args.push(Value::Integer(page.offset));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(paged_args.iter()), |row| {
        let id: i64 = row.get(0)?;
        let folder_id: i64 = row.get(1)?;
        let rel_path: String = row.get(2)?;
        let filename: String = row.get(3)?;
        let ext: String = row.get(4)?;
        let size_bytes: i64 = row.get(5)?;
        let mtime_ms: i64 = row.get(6)?;
        let width: Option<i64> = row.get(7)?;
        let height: Option<i64> = row.get(8)?;
        let content_hash: Option<String> = row.get(9)?;
        let taken_at: Option<i64> = row.get(10)?;
        let title: Option<String> = row.get(11)?;

        let root = folder_paths.get(&folder_id).cloned().unwrap_or_default();
        let abs = if root.is_empty() {
            rel_path
        } else {
            PathBuf::from(&root)
                .join(&rel_path)
                .to_string_lossy()
                .into_owned()
        };
        Ok(ImageSummary {
            id,
            folder_id,
            path: abs,
            filename,
            ext,
            width,
            height,
            size_bytes,
            mtime_ms,
            taken_at,
            title,
            content_hash,
        })
    })?;
    let items = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(Page {
        items,
        total,
        offset: page.offset,
        limit: page.limit,
    })
}

/// Compose the `WHERE` clause used by both `query_images` and its
/// count sibling. Returns the SQL fragment plus the bound parameters.
fn build_where(filter: &ImageFilter) -> (String, Vec<Value>) {
    let mut clauses: Vec<String> = vec!["missing = 0".into()];
    let mut args: Vec<Value> = Vec::new();

    if let Some(ids) = &filter.folder_ids {
        if !ids.is_empty() {
            let placeholders = (0..ids.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            clauses.push(format!("folder_id IN ({})", placeholders));
            for id in ids {
                args.push(Value::Integer(*id));
            }
        }
    }
    if let Some(after) = filter.taken_after {
        clauses.push("taken_at >= ?".into());
        args.push(Value::Integer(after));
    }
    if let Some(before) = filter.taken_before {
        clauses.push("taken_at <= ?".into());
        args.push(Value::Integer(before));
    }
    if let Some(ext) = &filter.ext {
        if !ext.is_empty() {
            let placeholders = (0..ext.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            clauses.push(format!("ext IN ({})", placeholders));
            for e in ext {
                args.push(Value::Text(e.to_lowercase()));
            }
        }
    }
    if let Some(true) = filter.has_title {
        clauses.push("title IS NOT NULL AND title <> ''".into());
    }
    if let Some(fts) = &filter.fts {
        let q = fts.trim();
        if !q.is_empty() {
            clauses.push(
                "id IN (SELECT rowid FROM images_fts WHERE images_fts MATCH ?)".into(),
            );
            args.push(Value::Text(fts_query_from_user(q)));
        }
    }
    if let Some(all) = &filter.tags_all {
        for t in all {
            let name = t.trim();
            if name.is_empty() {
                continue;
            }
            clauses.push(
                "id IN (SELECT it.image_id FROM image_tags it
                        JOIN tags t ON t.id = it.tag_id
                        WHERE t.name = ? COLLATE NOCASE)"
                    .into(),
            );
            args.push(Value::Text(name.to_string()));
        }
    }
    if let Some(any) = &filter.tags_any {
        let names: Vec<&str> = any
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !names.is_empty() {
            let placeholders = (0..names.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            clauses.push(format!(
                "id IN (SELECT it.image_id FROM image_tags it
                        JOIN tags t ON t.id = it.tag_id
                        WHERE t.name IN ({}) COLLATE NOCASE)",
                placeholders
            ));
            for n in names {
                args.push(Value::Text(n.to_string()));
            }
        }
    }
    if let Some(none) = &filter.tags_none {
        for t in none {
            let name = t.trim();
            if name.is_empty() {
                continue;
            }
            clauses.push(
                "id NOT IN (SELECT it.image_id FROM image_tags it
                            JOIN tags t ON t.id = it.tag_id
                            WHERE t.name = ? COLLATE NOCASE)"
                    .into(),
            );
            args.push(Value::Text(name.to_string()));
        }
    }

    (clauses.join(" AND "), args)
}

/// Escape user text for FTS5. Wrap tokens in double quotes so special
/// characters are literal, and append `*` to the last token for prefix
/// search — mirrors the pre-redesign behaviour.
fn fts_query_from_user(s: &str) -> String {
    let tokens: Vec<String> = s
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = tokens.iter().map(|t| format!("\"{}\"", t)).collect();
    if let Some(last) = parts.last_mut() {
        if last.ends_with('"') {
            last.pop();
        }
        last.push_str("\"*");
    }
    parts.join(" ")
}

// ---------------------------------------------------------------------
//                        Smart collections
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SmartCollectionRow {
    pub id: i64,
    pub name: String,
    pub filter: String,
    pub sort_order: i64,
}

pub fn list_smart_collections(conn: &Connection) -> AppResult<Vec<SmartCollectionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, filter, sort_order
         FROM smart_collections ORDER BY sort_order, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SmartCollectionRow {
            id: row.get(0)?,
            name: row.get(1)?,
            filter: row.get(2)?,
            sort_order: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn create_smart_collection(
    conn: &Connection,
    name: &str,
    filter: &str,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO smart_collections (name, filter, sort_order)
         VALUES (?1, ?2, COALESCE((SELECT MAX(sort_order) FROM smart_collections), -1) + 1)",
        params![name.trim(), filter],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_smart_collection(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM smart_collections WHERE id = ?1", params![id])?;
    Ok(())
}
