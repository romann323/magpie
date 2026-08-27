//! Registry DB — the per-machine control-plane database.
//!
//! Small (grows with number of registered folders, not files). Stores:
//! - `library_folders`   – which folders Magpie is watching
//! - `smart_collections` – saved filters (cross-folder)
//! - `app_settings`      – misc key/value store
//!
//! Every attached per-folder DB (`f1.images`, `f2.images`, …) lives
//! *inside* the folder as `<folder>/.magpie/library.db`; the registry
//! only knows the folder's *absolute path*, and the DB path is derived
//! from that.

use crate::db::pool::library_db_path_for;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub const REGISTRY_FILE_NAME: &str = "registry.db";

/// One row per registered library folder. Cross-references the per-folder
/// [`LibraryDb`](crate::db::library::LibraryDb) via `.magpie/library.db`
/// inside the folder.
#[derive(Debug, Clone)]
pub struct LibraryFolderRow {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub last_scan_at: Option<i64>,
    pub is_available: bool,
}

/// Applies every pending migration; safe to call on every launch.
pub fn init_registry_schema(conn: &Connection) -> AppResult<()> {
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
        "0001_init_registry",
        r#"
        CREATE TABLE library_folders (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            path          TEXT NOT NULL UNIQUE,
            added_at      INTEGER NOT NULL,
            last_scan_at  INTEGER,
            is_available  INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE smart_collections (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL,
            filter     TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE app_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
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
    tracing::info!(name, "applying registry migration");
    conn.execute_batch(sql)?;
    conn.execute(
        "INSERT INTO _migrations (name, applied_at) VALUES (?1, ?2)",
        params![name, chrono::Utc::now().timestamp_millis()],
    )?;
    Ok(())
}

// ------------- library_folders -------------

/// Insert a new folder if not present. Returns the row (existing or new).
pub fn insert_folder(conn: &Connection, path: &str) -> AppResult<LibraryFolderRow> {
    conn.execute(
        "INSERT OR IGNORE INTO library_folders (path, added_at) VALUES (?1, ?2)",
        params![path, chrono::Utc::now().timestamp_millis()],
    )?;
    get_folder_by_path(conn, path)
}

pub fn delete_folder(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute(
        "DELETE FROM library_folders WHERE id = ?1",
        params![id],
    )?;
    if n == 0 {
        return Err(AppError::FolderNotFound(id));
    }
    Ok(())
}

pub fn list_folders(conn: &Connection) -> AppResult<Vec<LibraryFolderRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, added_at, last_scan_at, is_available
         FROM library_folders
         ORDER BY added_at ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LibraryFolderRow {
            id: row.get(0)?,
            path: row.get(1)?,
            added_at: row.get(2)?,
            last_scan_at: row.get(3)?,
            is_available: row.get::<_, i64>(4)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_folder_by_path(conn: &Connection, path: &str) -> AppResult<LibraryFolderRow> {
    let mut stmt = conn.prepare(
        "SELECT id, path, added_at, last_scan_at, is_available
         FROM library_folders WHERE path = ?1",
    )?;
    stmt.query_row(params![path], |row| {
        Ok(LibraryFolderRow {
            id: row.get(0)?,
            path: row.get(1)?,
            added_at: row.get(2)?,
            last_scan_at: row.get(3)?,
            is_available: row.get::<_, i64>(4)? != 0,
        })
    })
    .map_err(Into::into)
}

pub fn get_folder(conn: &Connection, id: i64) -> AppResult<LibraryFolderRow> {
    conn.query_row(
        "SELECT id, path, added_at, last_scan_at, is_available
         FROM library_folders WHERE id = ?1",
        params![id],
        |row| {
            Ok(LibraryFolderRow {
                id: row.get(0)?,
                path: row.get(1)?,
                added_at: row.get(2)?,
                last_scan_at: row.get(3)?,
                is_available: row.get::<_, i64>(4)? != 0,
            })
        },
    )
    .optional()?
    .ok_or(AppError::FolderNotFound(id))
}

pub fn set_last_scan_at(conn: &Connection, folder_id: i64, ts: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE library_folders SET last_scan_at = ?1 WHERE id = ?2",
        params![ts, folder_id],
    )?;
    Ok(())
}

pub fn set_availability(conn: &Connection, folder_id: i64, available: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE library_folders SET is_available = ?1 WHERE id = ?2",
        params![if available { 1 } else { 0 }, folder_id],
    )?;
    Ok(())
}

/// Convenience: given a folder row, compute the on-disk path of its
/// per-folder library DB. The library DB lives at
/// `<folder>/.magpie/library.db` (see [`library_db_path_for`]).
pub fn library_db_path(folder: &LibraryFolderRow) -> PathBuf {
    library_db_path_for(Path::new(&folder.path))
}

// ------------- smart_collections -------------

/// JSON-encoded filter payload; parsing lives in `commands::collections`.
#[derive(Debug, Clone)]
pub struct SmartCollectionRow {
    pub id: i64,
    pub name: String,
    pub filter_json: String,
    pub sort_order: i64,
}

pub fn list_smart_collections(conn: &Connection) -> AppResult<Vec<SmartCollectionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, filter, sort_order FROM smart_collections
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SmartCollectionRow {
            id: row.get(0)?,
            name: row.get(1)?,
            filter_json: row.get(2)?,
            sort_order: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn create_smart_collection(
    conn: &Connection,
    name: &str,
    filter_json: &str,
) -> AppResult<SmartCollectionRow> {
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
    Ok(SmartCollectionRow {
        id: conn.last_insert_rowid(),
        name: name.to_string(),
        filter_json: filter_json.to_string(),
        sort_order,
    })
}

pub fn delete_smart_collection(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "DELETE FROM smart_collections WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
