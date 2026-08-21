use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::types::*;
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::collections::HashSet;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// -------- Library Folders --------

pub fn add_folder(db: &Db, path: &str) -> AppResult<LibraryFolder> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO library_folders (path, added_at) VALUES (?1, ?2)",
            params![path, now_ms()],
        )?;
        get_folder_by_path(conn, path)
    })
}

pub fn remove_folder(db: &Db, id: i64) -> AppResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute("DELETE FROM library_folders WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::FolderNotFound(id));
        }
        Ok(())
    })
}

pub fn list_folders(db: &Db) -> AppResult<Vec<LibraryFolder>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT f.id, f.path, f.added_at, f.last_scan_at,
                    (SELECT COUNT(*) FROM images i WHERE i.folder_id = f.id AND i.missing = 0)
             FROM library_folders f
             ORDER BY f.added_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LibraryFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                added_at: row.get(2)?,
                last_scan_at: row.get(3)?,
                image_count: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
}

pub fn set_last_scan_at(db: &Db, folder_id: i64, ts: i64) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE library_folders SET last_scan_at = ?1 WHERE id = ?2",
            params![ts, folder_id],
        )?;
        Ok(())
    })
}

fn get_folder_by_path(conn: &Connection, path: &str) -> AppResult<LibraryFolder> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.path, f.added_at, f.last_scan_at,
                (SELECT COUNT(*) FROM images i WHERE i.folder_id = f.id AND i.missing = 0)
         FROM library_folders f WHERE f.path = ?1",
    )?;
    stmt.query_row(params![path], |row| {
        Ok(LibraryFolder {
            id: row.get(0)?,
            path: row.get(1)?,
            added_at: row.get(2)?,
            last_scan_at: row.get(3)?,
            image_count: row.get(4)?,
        })
    })
    .map_err(Into::into)
}

// -------- Images --------

/// Data we know just from statting the file.
pub struct FileStat {
    pub folder_id: i64,
    pub path: String,
    pub filename: String,
    pub ext: String,
    pub size_bytes: i64,
    pub mtime_ms: i64,
}

/// Subset of on-disk file metadata we persist in the DB for search/sort.
/// The full technical metadata is regenerated on-demand for the DetailsPanel;
/// only sortable/filterable fields end up here.
#[derive(Default)]
pub struct ImageMetaFromFile {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub taken_at: Option<i64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

pub fn get_image_paths(db: &Db, ids: &[i64]) -> AppResult<Vec<(i64, String)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    db.with_conn(|conn| {
        let placeholders = (0..ids.len())
            .map(|_| "?".to_string())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, path FROM images WHERE id IN ({})",
            placeholders
        );
        let args: Vec<Value> = ids.iter().map(|i| Value::Integer(*i)).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
}

pub fn delete_image_rows(db: &Db, ids: &[i64]) -> AppResult<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        let mut removed = 0usize;
        for id in ids {
            let n = tx.execute("DELETE FROM images WHERE id = ?1", params![id])?;
            let _ = tx.execute("DELETE FROM images_fts WHERE rowid = ?1", params![id]);
            removed += n;
        }
        tx.commit()?;
        Ok(removed)
    })
}

pub fn image_exists_by_path(db: &Db, path: &str) -> AppResult<bool> {
    db.with_conn(|conn| {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM images WHERE path = ?1",
                params![path],
                |r| r.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    })
}

pub fn upsert_image_stat(db: &Db, s: &FileStat) -> AppResult<(i64, bool)> {
    db.with_conn(|conn| {
        let existing: Option<(i64, i64, i64)> = conn
            .query_row(
                "SELECT id, size_bytes, mtime_ms FROM images WHERE path = ?1",
                params![s.path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        match existing {
            Some((id, size, mtime)) => {
                let unchanged = size == s.size_bytes && mtime == s.mtime_ms;
                if unchanged {
                    conn.execute("UPDATE images SET missing = 0 WHERE id = ?1", params![id])?;
                    Ok((id, false))
                } else {
                    conn.execute(
                        "UPDATE images
                         SET folder_id = ?1, filename = ?2, ext = ?3,
                             size_bytes = ?4, mtime_ms = ?5, missing = 0
                         WHERE id = ?6",
                        params![
                            s.folder_id,
                            s.filename,
                            s.ext,
                            s.size_bytes,
                            s.mtime_ms,
                            id
                        ],
                    )?;
                    Ok((id, true))
                }
            }
            None => {
                conn.execute(
                    "INSERT INTO images
                     (folder_id, path, filename, ext, size_bytes, mtime_ms, missing)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
                    params![
                        s.folder_id,
                        s.path,
                        s.filename,
                        s.ext,
                        s.size_bytes,
                        s.mtime_ms
                    ],
                )?;
                let id = conn.last_insert_rowid();
                Ok((id, true))
            }
        }
    })
}

/// Overwrites the user-metadata columns for one image with values read from
/// the filesystem. Also updates `meta_read_at`.
///
/// Unlike [`set_image_meta`], this takes the FS as authoritative for the
/// title/tags fields (they will be `NULL`ed if the file no longer carries
/// them). Technical fields still use `COALESCE` — dimensions/taken_at/camera
/// don't need to be periodically re-blanked.
pub fn resync_user_meta_from_fs(
    db: &Db,
    image_id: i64,
    m: &ImageMetaFromFile,
) -> AppResult<()> {
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE images SET
                width        = COALESCE(?1, width),
                height       = COALESCE(?2, height),
                taken_at     = COALESCE(?3, taken_at),
                camera_make  = COALESCE(?4, camera_make),
                camera_model = COALESCE(?5, camera_model),
                title        = ?6,
                meta_read_at = ?7
             WHERE id = ?8",
            params![
                m.width,
                m.height,
                m.taken_at,
                m.camera_make,
                m.camera_model,
                m.title,
                now_ms(),
                image_id,
            ],
        )?;
        replace_image_tags_tx(&tx, image_id, &m.tags)?;
        rebuild_fts_row_tx(&tx, image_id)?;
        tx.commit()?;
        Ok(())
    })
}

pub fn set_image_meta(db: &Db, image_id: i64, m: &ImageMetaFromFile) -> AppResult<()> {
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE images SET
                width = COALESCE(?1, width),
                height = COALESCE(?2, height),
                taken_at = COALESCE(?3, taken_at),
                camera_make = COALESCE(?4, camera_make),
                camera_model = COALESCE(?5, camera_model),
                title = ?6,
                meta_read_at = ?7
             WHERE id = ?8",
            params![
                m.width,
                m.height,
                m.taken_at,
                m.camera_make,
                m.camera_model,
                m.title,
                now_ms(),
                image_id,
            ],
        )?;

        replace_image_tags_tx(&tx, image_id, &m.tags)?;
        rebuild_fts_row_tx(&tx, image_id)?;
        tx.commit()?;
        Ok(())
    })
}

pub fn set_image_content_hash(db: &Db, image_id: i64, hash: &str) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE images SET content_hash = ?1 WHERE id = ?2",
            params![hash, image_id],
        )?;
        Ok(())
    })
}

/// DB-only slice of ImageDetails: no technical metadata, no format-handler
/// info. commands/images.rs enriches this into a full ImageDetails using the
/// FormatRegistry.
pub struct ImageDetailsRow {
    pub summary: ImageSummary,
    pub tags: Vec<String>,
    pub meta_written_at: Option<i64>,
    pub meta_read_at: Option<i64>,
}

pub fn get_image_row(db: &Db, image_id: i64) -> AppResult<ImageDetailsRow> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at,
                    title, meta_written_at, meta_read_at
             FROM images WHERE id = ?1",
        )?;
        let (summary, mw, mr) = stmt
            .query_row(params![image_id], |row| {
                Ok((
                    ImageSummary {
                        id: row.get(0)?,
                        folder_id: row.get(1)?,
                        path: row.get(2)?,
                        filename: row.get(3)?,
                        ext: row.get(4)?,
                        size_bytes: row.get(5)?,
                        mtime_ms: row.get(6)?,
                        width: row.get(7)?,
                        height: row.get(8)?,
                        content_hash: row.get(9)?,
                        taken_at: row.get(10)?,
                        title: row.get(11)?,
                    },
                    row.get::<_, Option<i64>>(12)?,
                    row.get::<_, Option<i64>>(13)?,
                ))
            })
            .optional()?
            .ok_or(AppError::ImageNotFound(image_id))?;

        let tags = get_tags_for_image(conn, image_id)?;

        Ok(ImageDetailsRow {
            summary,
            tags,
            meta_written_at: mw,
            meta_read_at: mr,
        })
    })
}

pub fn get_tags_for_image(conn: &Connection, image_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN image_tags it ON it.tag_id = t.id
         WHERE it.image_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![image_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn mark_folder_paths_missing(db: &Db, folder_id: i64, seen: &HashSet<String>) -> AppResult<i64> {
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        let mut removed = 0i64;
        {
            let mut stmt = tx.prepare(
                "SELECT id, path FROM images WHERE folder_id = ?1 AND missing = 0",
            )?;
            let rows: Vec<(i64, String)> = stmt
                .query_map(params![folder_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, p) in rows {
                if !seen.contains(&p) {
                    tx.execute("UPDATE images SET missing = 1 WHERE id = ?1", params![id])?;
                    removed += 1;
                }
            }
        }
        tx.commit()?;
        Ok(removed)
    })
}

// -------- Metadata patches (from UI) --------

pub fn apply_metadata_patch(db: &Db, image_id: i64, patch: &MetadataPatch) -> AppResult<()> {
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;

        if let Some(title) = &patch.title {
            tx.execute(
                "UPDATE images SET title = ?1 WHERE id = ?2",
                params![title, image_id],
            )?;
        }
        if let Some(tags) = &patch.tags {
            replace_image_tags_tx(&tx, image_id, tags)?;
        }
        if let Some(add) = &patch.tags_add {
            for t in add {
                if !t.trim().is_empty() {
                    add_tag_to_image_tx(&tx, image_id, t)?;
                }
            }
        }
        if let Some(rm) = &patch.tags_remove {
            for t in rm {
                remove_tag_from_image_tx(&tx, image_id, t)?;
            }
        }

        rebuild_fts_row_tx(&tx, image_id)?;
        tx.commit()?;
        Ok(())
    })
}

pub fn set_meta_written_at(db: &Db, image_id: i64, ts: i64) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE images SET meta_written_at = ?1 WHERE id = ?2",
            params![ts, image_id],
        )?;
        Ok(())
    })
}

pub fn set_meta_read_at_now(db: &Db, image_id: i64) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE images SET meta_read_at = ?1 WHERE id = ?2",
            params![now_ms(), image_id],
        )?;
        Ok(())
    })
}

// -------- Tags --------

fn tag_id_for_name(conn: &Connection, name: &str) -> AppResult<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
        params![name.trim()],
    )?;
    Ok(conn.query_row(
        "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
        params![name.trim()],
        |row| row.get(0),
    )?)
}

fn add_tag_to_image_tx(tx: &rusqlite::Transaction, image_id: i64, name: &str) -> AppResult<()> {
    let tag_id = tag_id_for_name(tx, name)?;
    tx.execute(
        "INSERT OR IGNORE INTO image_tags (image_id, tag_id) VALUES (?1, ?2)",
        params![image_id, tag_id],
    )?;
    Ok(())
}

fn remove_tag_from_image_tx(
    tx: &rusqlite::Transaction,
    image_id: i64,
    name: &str,
) -> AppResult<()> {
    tx.execute(
        "DELETE FROM image_tags
         WHERE image_id = ?1
           AND tag_id = (SELECT id FROM tags WHERE name = ?2 COLLATE NOCASE)",
        params![image_id, name.trim()],
    )?;
    Ok(())
}

fn replace_image_tags_tx(
    tx: &rusqlite::Transaction,
    image_id: i64,
    tags: &[String],
) -> AppResult<()> {
    tx.execute(
        "DELETE FROM image_tags WHERE image_id = ?1",
        params![image_id],
    )?;
    for t in tags {
        let name = t.trim();
        if name.is_empty() {
            continue;
        }
        let tag_id = tag_id_for_name(tx, name)?;
        tx.execute(
            "INSERT OR IGNORE INTO image_tags (image_id, tag_id) VALUES (?1, ?2)",
            params![image_id, tag_id],
        )?;
    }
    Ok(())
}

pub fn list_tags(db: &Db, prefix: Option<&str>) -> AppResult<Vec<TagStats>> {
    db.with_conn(|conn| {
        let (sql, args): (&str, Vec<Value>) = match prefix {
            Some(p) if !p.trim().is_empty() => (
                "SELECT t.name, COUNT(it.image_id) as c
                 FROM tags t
                 LEFT JOIN image_tags it ON it.tag_id = t.id
                 WHERE t.name LIKE ?1 COLLATE NOCASE
                 GROUP BY t.id ORDER BY c DESC, t.name COLLATE NOCASE
                 LIMIT 200",
                vec![Value::Text(format!("{}%", p))],
            ),
            _ => (
                "SELECT t.name, COUNT(it.image_id) as c
                 FROM tags t
                 LEFT JOIN image_tags it ON it.tag_id = t.id
                 GROUP BY t.id ORDER BY c DESC, t.name COLLATE NOCASE
                 LIMIT 500",
                vec![],
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
            Ok(TagStats {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
}

pub fn rename_tag(db: &Db, old: &str, new: &str) -> AppResult<()> {
    let new = new.trim();
    if new.is_empty() {
        return Err(AppError::BadInput("new tag name is empty".into()));
    }
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        let updated = tx.execute(
            "UPDATE OR IGNORE tags SET name = ?1 WHERE name = ?2 COLLATE NOCASE",
            params![new, old.trim()],
        )?;
        if updated == 0 {
            // Merge: link everything from `old` into `new`, drop `old`
            let old_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                    params![old.trim()],
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
                    tx.execute(
                        "INSERT OR IGNORE INTO image_tags (image_id, tag_id)
                         SELECT image_id, ?2 FROM image_tags WHERE tag_id = ?1",
                        params![o, n],
                    )?;
                    tx.execute("DELETE FROM tags WHERE id = ?1", params![o])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn delete_tag(db: &Db, name: &str) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "DELETE FROM tags WHERE name = ?1 COLLATE NOCASE",
            params![name.trim()],
        )?;
        Ok(())
    })
}

// -------- FTS --------

fn rebuild_fts_row_tx(tx: &rusqlite::Transaction, image_id: i64) -> AppResult<()> {
    tx.execute("DELETE FROM images_fts WHERE rowid = ?1", params![image_id])?;
    let row: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT filename, title FROM images WHERE id = ?1",
            params![image_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((filename, title)) = row {
        let tags = get_tags_for_image(tx, image_id)?.join(" ");
        tx.execute(
            "INSERT INTO images_fts(rowid, title, filename, tags)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                image_id,
                title.unwrap_or_default(),
                filename,
                tags
            ],
        )?;
    }
    Ok(())
}

// -------- Query images --------

pub fn query_images(
    db: &Db,
    filter: &ImageFilter,
    sort: &ImageSort,
    page: &Pagination,
) -> AppResult<Page<ImageSummary>> {
    let mut where_clauses: Vec<String> = vec!["i.missing = 0".into()];
    let mut args: Vec<Value> = Vec::new();

    if let Some(ids) = &filter.folder_ids {
        if !ids.is_empty() {
            let placeholders = (0..ids.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            where_clauses.push(format!("i.folder_id IN ({})", placeholders));
            for id in ids {
                args.push(Value::Integer(*id));
            }
        }
    }
    if let Some(after) = filter.taken_after {
        where_clauses.push("i.taken_at >= ?".into());
        args.push(Value::Integer(after));
    }
    if let Some(before) = filter.taken_before {
        where_clauses.push("i.taken_at <= ?".into());
        args.push(Value::Integer(before));
    }
    if let Some(ext) = &filter.ext {
        if !ext.is_empty() {
            let placeholders = (0..ext.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            where_clauses.push(format!("i.ext IN ({})", placeholders));
            for e in ext {
                args.push(Value::Text(e.to_lowercase()));
            }
        }
    }
    if let Some(true) = filter.has_title {
        where_clauses.push("i.title IS NOT NULL AND i.title <> ''".into());
    }
    if let Some(fts) = &filter.fts {
        let s = fts.trim();
        if !s.is_empty() {
            where_clauses.push(
                "i.id IN (SELECT rowid FROM images_fts WHERE images_fts MATCH ?)".into(),
            );
            args.push(Value::Text(fts_query_from_user(s)));
        }
    }
    if let Some(all) = &filter.tags_all {
        for t in all {
            let name = t.trim();
            if name.is_empty() {
                continue;
            }
            where_clauses.push(
                "i.id IN (SELECT it.image_id FROM image_tags it
                          JOIN tags t ON t.id = it.tag_id
                          WHERE t.name = ? COLLATE NOCASE)"
                    .into(),
            );
            args.push(Value::Text(name.to_string()));
        }
    }
    if let Some(any) = &filter.tags_any {
        let names: Vec<&str> = any.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if !names.is_empty() {
            let placeholders = (0..names.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            where_clauses.push(format!(
                "i.id IN (SELECT it.image_id FROM image_tags it
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
            where_clauses.push(
                "i.id NOT IN (SELECT it.image_id FROM image_tags it
                              JOIN tags t ON t.id = it.tag_id
                              WHERE t.name = ? COLLATE NOCASE)"
                    .into(),
            );
            args.push(Value::Text(name.to_string()));
        }
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let order_by = match (sort.by, sort.dir) {
        (SortBy::TakenAt, SortDir::Asc) => "COALESCE(i.taken_at, i.mtime_ms) ASC, i.id ASC",
        (SortBy::TakenAt, SortDir::Desc) => "COALESCE(i.taken_at, i.mtime_ms) DESC, i.id DESC",
        (SortBy::Filename, SortDir::Asc) => "i.filename COLLATE NOCASE ASC, i.id ASC",
        (SortBy::Filename, SortDir::Desc) => "i.filename COLLATE NOCASE DESC, i.id DESC",
        (SortBy::AddedAt, SortDir::Asc) => "i.id ASC",
        (SortBy::AddedAt, SortDir::Desc) => "i.id DESC",
        (SortBy::Size, SortDir::Asc) => "i.size_bytes ASC, i.id ASC",
        (SortBy::Size, SortDir::Desc) => "i.size_bytes DESC, i.id DESC",
    };

    db.with_conn(|conn| {
        let count_sql = format!("SELECT COUNT(*) FROM images i {}", where_sql);
        let total: i64 = conn.query_row(&count_sql, params_from_iter(args.iter()), |r| r.get(0))?;

        let sql = format!(
            "SELECT i.id, i.folder_id, i.path, i.filename, i.ext, i.size_bytes, i.mtime_ms,
                    i.width, i.height, i.content_hash, i.taken_at, i.title
             FROM images i
             {}
             ORDER BY {}
             LIMIT ? OFFSET ?",
            where_sql, order_by
        );

        let mut final_args = args.clone();
        final_args.push(Value::Integer(page.limit));
        final_args.push(Value::Integer(page.offset));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(final_args.iter()), |row| {
            Ok(ImageSummary {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                path: row.get(2)?,
                filename: row.get(3)?,
                ext: row.get(4)?,
                size_bytes: row.get(5)?,
                mtime_ms: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                content_hash: row.get(9)?,
                taken_at: row.get(10)?,
                title: row.get(11)?,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(Page {
            items,
            total,
            offset: page.offset,
            limit: page.limit,
        })
    })
}

/// Escape user text for FTS5. We wrap tokens in quotes to allow special chars,
/// and append a `*` to the last token for prefix search.
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

// -------- Smart collections --------

pub fn list_smart_collections(db: &Db) -> AppResult<Vec<SmartCollection>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, filter, sort_order FROM smart_collections
             ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let filter_json: String = row.get(2)?;
            let filter: ImageFilter =
                serde_json::from_str(&filter_json).unwrap_or_default();
            Ok(SmartCollection {
                id: row.get(0)?,
                name: row.get(1)?,
                filter,
                sort_order: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
}

pub fn create_smart_collection(
    db: &Db,
    name: &str,
    filter: &ImageFilter,
) -> AppResult<SmartCollection> {
    db.with_conn(|conn| {
        let filter_json = serde_json::to_string(filter).unwrap_or_else(|_| "{}".into());
        let sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM smart_collections",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO smart_collections (name, filter, sort_order) VALUES (?1, ?2, ?3)",
            params![name, filter_json, sort_order],
        )?;
        let id = conn.last_insert_rowid();
        Ok(SmartCollection {
            id,
            name: name.into(),
            filter: filter.clone(),
            sort_order,
        })
    })
}

pub fn delete_smart_collection(db: &Db, id: i64) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.execute("DELETE FROM smart_collections WHERE id = ?1", params![id])?;
        Ok(())
    })
}
