//! Per-folder library DB. One SQLite file per registered folder, stored
//! at `<folder>/.magpie/library.db`. Fully self-contained: paths are
//! folder-relative, tag names are stored inline, and the FTS index sits
//! next to the tags table.
//!
//! Concurrency: writes to one folder serialize through the folder's own
//! `Connection` (wrapped in a `Mutex`). Different folders write in
//! parallel.
//!
//! WAL mode is enabled per DB, so if another process (Explorer, a
//! backup tool, …) reads `library.db` while Magpie is writing, no
//! reader blocks.

use crate::error::{AppError, AppResult};
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Schema version stored in `folder_meta.schema_version`. Bump when
/// making a breaking change; add a corresponding migration below.
pub const SCHEMA_VERSION: i64 = 1;

/// One instance per registered folder. Owns a single writer connection
/// under a `Mutex` — cheap because writes are always small (tag edit,
/// single-file upsert).
pub struct LibraryDb {
    conn: Arc<Mutex<Connection>>,
    folder_id: i64,
}

impl LibraryDb {
    /// Open or create the library DB for a folder. Runs schema
    /// migrations. `folder_id` is the row ID in the central registry.
    pub fn open(db_path: &Path, folder_id: i64) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        // WAL: readers don't block writers, and vice-versa. Critical
        // for the read-through cross-folder search path — a background
        // scan on folder A shouldn't stall a search that also touches
        // folder A.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;

        init_library_schema(&conn)?;
        ensure_folder_meta_row(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            folder_id,
        })
    }

    pub fn folder_id(&self) -> i64 {
        self.folder_id
    }

    /// Grab the writer connection. Callers should keep the guard as
    /// short as possible; other operations on this folder will block.
    pub fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::Pool("library DB mutex poisoned".into()))
    }
}

fn ensure_folder_meta_row(conn: &Connection) -> AppResult<()> {
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM folder_meta WHERE id = 1", [], |r| r.get(0))
        .optional()?;
    if existing.is_none() {
        conn.execute(
            "INSERT INTO folder_meta (id, magpie_version, schema_version, created_at)
             VALUES (1, ?1, ?2, ?3)",
            params![
                env!("CARGO_PKG_VERSION"),
                SCHEMA_VERSION,
                chrono::Utc::now().timestamp_millis(),
            ],
        )?;
    }
    Ok(())
}

fn init_library_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            name       TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );
        "#,
    )?;
    apply_migration(
        conn,
        "0001_init_library",
        r#"
        CREATE TABLE folder_meta (
            id              INTEGER PRIMARY KEY CHECK (id = 1),
            magpie_version  TEXT NOT NULL,
            schema_version  INTEGER NOT NULL,
            created_at      INTEGER NOT NULL,
            last_scan_at    INTEGER
        );

        CREATE TABLE images (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            rel_path      TEXT NOT NULL UNIQUE,
            filename      TEXT NOT NULL,
            ext           TEXT NOT NULL,
            size_bytes    INTEGER NOT NULL,
            mtime_ms      INTEGER NOT NULL,
            width         INTEGER,
            height        INTEGER,
            content_hash  TEXT,
            taken_at      INTEGER,
            camera_make   TEXT,
            camera_model  TEXT,
            title         TEXT,
            imported_at   INTEGER NOT NULL,
            missing       INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_images_taken_at ON images(taken_at);
        CREATE INDEX idx_images_filename ON images(filename);
        CREATE INDEX idx_images_ext      ON images(ext);

        CREATE TABLE tags (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE
        );

        CREATE TABLE image_tags (
            image_id INTEGER NOT NULL REFERENCES images(id) ON DELETE CASCADE,
            tag_id   INTEGER NOT NULL REFERENCES tags(id)   ON DELETE CASCADE,
            PRIMARY KEY (image_id, tag_id)
        );
        CREATE INDEX idx_image_tags_tag ON image_tags(tag_id);

        CREATE VIRTUAL TABLE images_fts USING fts5(
            title, filename, tags,
            content='',
            contentless_delete=1,
            tokenize='unicode61 remove_diacritics 2'
        );
        "#,
    )
}

fn apply_migration(conn: &Connection, name: &str, sql: &str) -> AppResult<()> {
    let already: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM _migrations WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .optional()?;
    if already.is_some() {
        return Ok(());
    }
    tracing::info!(name, "applying library migration");
    conn.execute_batch(sql)?;
    conn.execute(
        "INSERT INTO _migrations (name, applied_at) VALUES (?1, ?2)",
        params![name, chrono::Utc::now().timestamp_millis()],
    )?;
    Ok(())
}

// ---------- image CRUD ----------

/// Minimum fields the scanner produces from a `stat` call.
pub struct FileStat<'a> {
    pub rel_path: &'a str,
    pub filename: &'a str,
    pub ext: &'a str,
    pub size_bytes: i64,
    pub mtime_ms: i64,
}

pub enum UpsertOutcome {
    Added { local_id: i64 },
    Updated { local_id: i64 },
    Unchanged { local_id: i64 },
}

impl UpsertOutcome {
    /// The row's local (per-folder) primary key, regardless of
    /// which variant this outcome is.
    pub fn local_id(&self) -> i64 {
        match self {
            UpsertOutcome::Added { local_id }
            | UpsertOutcome::Updated { local_id }
            | UpsertOutcome::Unchanged { local_id } => *local_id,
        }
    }
}

pub fn upsert_image(conn: &Connection, s: &FileStat<'_>) -> AppResult<UpsertOutcome> {
    let existing: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, size_bytes, mtime_ms FROM images WHERE rel_path = ?1",
            params![s.rel_path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    match existing {
        Some((id, sz, mt)) => {
            if sz == s.size_bytes && mt == s.mtime_ms {
                conn.execute(
                    "UPDATE images SET missing = 0 WHERE id = ?1",
                    params![id],
                )?;
                Ok(UpsertOutcome::Unchanged { local_id: id })
            } else {
                conn.execute(
                    "UPDATE images
                       SET filename = ?1, ext = ?2, size_bytes = ?3, mtime_ms = ?4, missing = 0
                     WHERE id = ?5",
                    params![s.filename, s.ext, s.size_bytes, s.mtime_ms, id],
                )?;
                Ok(UpsertOutcome::Updated { local_id: id })
            }
        }
        None => {
            conn.execute(
                "INSERT INTO images
                   (rel_path, filename, ext, size_bytes, mtime_ms, imported_at, missing)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
                params![
                    s.rel_path,
                    s.filename,
                    s.ext,
                    s.size_bytes,
                    s.mtime_ms,
                    chrono::Utc::now().timestamp_millis()
                ],
            )?;
            Ok(UpsertOutcome::Added {
                local_id: conn.last_insert_rowid(),
            })
        }
    }
}

/// Subset of on-disk metadata the DB indexes. Full technical metadata
/// (EXIF, GPS, GPS references, video duration, …) is regenerated on
/// demand by `commands::images::enrich_details`.
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

pub fn set_image_meta(
    conn: &mut Connection,
    local_id: i64,
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
            local_id,
        ],
    )?;
    replace_image_tags_tx(&tx, local_id, &m.tags)?;
    rebuild_fts_row_tx(&tx, local_id)?;
    tx.commit()?;
    Ok(())
}

pub fn set_content_hash(conn: &Connection, local_id: i64, hash: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE images SET content_hash = ?1 WHERE id = ?2",
        params![hash, local_id],
    )?;
    Ok(())
}

pub fn set_folder_last_scan_at(conn: &Connection, ts: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE folder_meta SET last_scan_at = ?1 WHERE id = 1",
        params![ts],
    )?;
    Ok(())
}

pub fn mark_missing_by_seen(
    conn: &mut Connection,
    seen_rel_paths: &std::collections::HashSet<String>,
) -> AppResult<i64> {
    let tx = conn.transaction()?;
    let mut removed = 0i64;
    {
        let mut stmt = tx.prepare("SELECT id, rel_path FROM images WHERE missing = 0")?;
        let rows: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, p) in rows {
            if !seen_rel_paths.contains(&p) {
                tx.execute("UPDATE images SET missing = 1 WHERE id = ?1", params![id])?;
                removed += 1;
            }
        }
    }
    tx.commit()?;
    Ok(removed)
}

/// Full row for the DetailsPanel. Path is *relative* — callers need the
/// folder root from the registry to build an absolute path.
pub struct ImageRow {
    pub local_id: i64,
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
    pub tags: Vec<String>,
}

pub fn get_image_row(conn: &Connection, local_id: i64) -> AppResult<Option<ImageRow>> {
    let base: Option<ImageRow> = conn
        .query_row(
            "SELECT id, rel_path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at, title, imported_at
             FROM images WHERE id = ?1",
            params![local_id],
            |row| {
                Ok(ImageRow {
                    local_id: row.get(0)?,
                    rel_path: row.get(1)?,
                    filename: row.get(2)?,
                    ext: row.get(3)?,
                    size_bytes: row.get(4)?,
                    mtime_ms: row.get(5)?,
                    width: row.get(6)?,
                    height: row.get(7)?,
                    content_hash: row.get(8)?,
                    taken_at: row.get(9)?,
                    title: row.get(10)?,
                    imported_at: row.get(11)?,
                    tags: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(mut row) = base else {
        return Ok(None);
    };
    row.tags = tags_for_image(conn, local_id)?;
    Ok(Some(row))
}

pub fn tags_for_image(conn: &Connection, local_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN image_tags it ON it.tag_id = t.id
         WHERE it.image_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![local_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn delete_images(conn: &mut Connection, local_ids: &[i64]) -> AppResult<usize> {
    if local_ids.is_empty() {
        return Ok(0);
    }
    let tx = conn.transaction()?;
    let mut removed = 0;
    for id in local_ids {
        removed += tx.execute("DELETE FROM images WHERE id = ?1", params![id])?;
        let _ = tx.execute("DELETE FROM images_fts WHERE rowid = ?1", params![id]);
    }
    tx.commit()?;
    Ok(removed)
}

/// Absolute image paths (rel + folder root joined by the caller). Used
/// by `delete_images` to look up files to trash.
pub fn get_rel_paths(conn: &Connection, local_ids: &[i64]) -> AppResult<Vec<(i64, String)>> {
    if local_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (0..local_ids.len())
        .map(|_| "?".to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, rel_path FROM images WHERE id IN ({})",
        placeholders
    );
    let args: Vec<Value> = local_ids.iter().map(|i| Value::Integer(*i)).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------- Tag mutations ----------

pub struct MetadataPatch {
    /// `None`  = no change.
    /// `Some(None)`     = clear title.
    /// `Some(Some(s))`  = set title.
    pub title: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub tags_add: Option<Vec<String>>,
    pub tags_remove: Option<Vec<String>>,
}

pub fn apply_metadata_patch(
    conn: &mut Connection,
    local_id: i64,
    patch: &MetadataPatch,
) -> AppResult<()> {
    let tx = conn.transaction()?;
    if let Some(title) = &patch.title {
        tx.execute(
            "UPDATE images SET title = ?1 WHERE id = ?2",
            params![title, local_id],
        )?;
    }
    if let Some(tags) = &patch.tags {
        replace_image_tags_tx(&tx, local_id, tags)?;
    }
    if let Some(add) = &patch.tags_add {
        for t in add {
            let name = t.trim();
            if !name.is_empty() {
                add_tag_tx(&tx, local_id, name)?;
            }
        }
    }
    if let Some(rm) = &patch.tags_remove {
        for t in rm {
            remove_tag_tx(&tx, local_id, t.trim())?;
        }
    }
    rebuild_fts_row_tx(&tx, local_id)?;
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

fn add_tag_tx(tx: &rusqlite::Transaction, local_id: i64, name: &str) -> AppResult<()> {
    let tag_id = tag_id_for_name_tx(tx, name)?;
    tx.execute(
        "INSERT OR IGNORE INTO image_tags (image_id, tag_id) VALUES (?1, ?2)",
        params![local_id, tag_id],
    )?;
    Ok(())
}

fn remove_tag_tx(tx: &rusqlite::Transaction, local_id: i64, name: &str) -> AppResult<()> {
    if name.is_empty() {
        return Ok(());
    }
    tx.execute(
        "DELETE FROM image_tags
         WHERE image_id = ?1
           AND tag_id = (SELECT id FROM tags WHERE name = ?2 COLLATE NOCASE)",
        params![local_id, name],
    )?;
    Ok(())
}

fn replace_image_tags_tx(
    tx: &rusqlite::Transaction,
    local_id: i64,
    tags: &[String],
) -> AppResult<()> {
    tx.execute(
        "DELETE FROM image_tags WHERE image_id = ?1",
        params![local_id],
    )?;
    for t in tags {
        let name = t.trim();
        if name.is_empty() {
            continue;
        }
        add_tag_tx(tx, local_id, name)?;
    }
    Ok(())
}

fn rebuild_fts_row_tx(tx: &rusqlite::Transaction, local_id: i64) -> AppResult<()> {
    tx.execute(
        "DELETE FROM images_fts WHERE rowid = ?1",
        params![local_id],
    )?;
    let row: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT filename, title FROM images WHERE id = ?1",
            params![local_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((filename, title)) = row {
        // Tags need to come from the current transaction, not a fresh
        // query — otherwise we'd see the pre-DELETE state.
        let mut stmt = tx.prepare(
            "SELECT t.name FROM tags t
             JOIN image_tags it ON it.tag_id = t.id
             WHERE it.image_id = ?1",
        )?;
        let tags: Vec<String> = stmt
            .query_map(params![local_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        let joined = tags.join(" ");
        tx.execute(
            "INSERT INTO images_fts(rowid, title, filename, tags)
             VALUES (?1, ?2, ?3, ?4)",
            params![local_id, title.unwrap_or_default(), filename, joined],
        )?;
    }
    Ok(())
}

/// Rename or merge a tag inside one library. Merges when `new_name`
/// already exists — every image gets the new tag, the old tag row is
/// dropped.
pub fn rename_tag(conn: &mut Connection, old: &str, new: &str) -> AppResult<()> {
    let new = new.trim();
    if new.is_empty() {
        return Err(AppError::BadInput("new tag name is empty".into()));
    }
    let tx = conn.transaction()?;
    let updated = tx.execute(
        "UPDATE OR IGNORE tags SET name = ?1 WHERE name = ?2 COLLATE NOCASE",
        params![new, old.trim()],
    )?;
    if updated == 0 {
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
    // FTS rows still index the old tag names — rebuild everything.
    let mut stmt = tx.prepare(
        "SELECT DISTINCT it.image_id FROM image_tags it
         JOIN tags t ON t.id = it.tag_id
         WHERE t.name = ?1 COLLATE NOCASE",
    )?;
    let affected: Vec<i64> = stmt
        .query_map(params![new], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for id in affected {
        rebuild_fts_row_tx(&tx, id)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn delete_tag(conn: &mut Connection, name: &str) -> AppResult<()> {
    let name = name.trim();
    let tx = conn.transaction()?;
    let mut stmt = tx.prepare(
        "SELECT DISTINCT it.image_id FROM image_tags it
         JOIN tags t ON t.id = it.tag_id
         WHERE t.name = ?1 COLLATE NOCASE",
    )?;
    let affected: Vec<i64> = stmt
        .query_map(params![name], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
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
