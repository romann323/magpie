//! Single-database storage layer.
//!
//! Everything Magpie persists — registered folders, image rows, tags,
//! FTS index, smart collections, app settings — lives in one SQLite
//! file at `%APPDATA%\com.magpie.app\magpie.db`.
//!
//! Layout:
//!
//! - [`Db`] — thin, `Send + Sync` wrapper around a single
//!   `rusqlite::Connection` guarded by a `Mutex`. WAL mode is enabled
//!   at open time so external readers (e.g. DB Browser, the diagnostic
//!   examples) don't block Magpie's own writes.
//! - [`schema`] — the DDL applied on a fresh DB.
//! - [`queries`] — every SQL operation Magpie needs, grouped by
//!   concern (folders, images, tags, search, smart collections).
//! - [`migrate`] — one-shot importer for the two legacy layouts (the
//!   original central `library.db` and the intermediate per-folder
//!   `.magpie/library.db` files).

pub mod migrate;
pub mod queries;
pub mod schema;

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Filename of the central Magpie database inside the app data dir.
pub const DB_FILE_NAME: &str = "magpie.db";

/// Current schema version. Bump when adding a migration inside
/// [`schema::apply`] / a future `migrate_up` helper.
///
/// - v1: initial single-DB layout.
/// - v2: `image_tags.source` column ('auto' | 'user'); PK becomes
///   `(image_id, tag_id, source)`.
/// - v3: `images.ai_tagged_at` and `images.ai_tag_hash` columns for
///   automatic-AI-tagging bookkeeping.
pub const SCHEMA_VERSION: i64 = 3;

/// Thread-safe handle to Magpie's single SQLite database.
///
/// Cheap to clone (it wraps `Arc<Mutex<Connection>>`). All queries go
/// through [`Db::with_conn`] / [`Db::with_conn_mut`] so the mutex is
/// held for the shortest possible window.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    /// Open (or create) the DB at `path`, apply the schema if the file
    /// is fresh, and set up the connection pragmas.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;

        schema::apply(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run a closure with a read/write borrow on the underlying
    /// connection. Panicking closures don't poison the DB because the
    /// mutex guard is released the moment the closure returns.
    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self.lock()?;
        f(&guard)
    }

    /// Like [`Db::with_conn`] but hands the closure a `&mut Connection`
    /// so it can start a transaction.
    pub fn with_conn_mut<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut guard = self.lock()?;
        f(&mut guard)
    }

    fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::Pool("magpie.db mutex poisoned".into()))
    }
}
