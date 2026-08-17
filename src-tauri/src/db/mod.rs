pub mod migrations;
pub mod queries;

use crate::error::{PicOrgError, PicOrgResult};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Simple single-writer, many-reader wrapper around a SQLite connection.
/// SQLite in WAL mode already handles concurrency well; we serialize
/// writes through a Mutex to avoid busy-loops.
#[derive(Clone)]
pub struct Db {
    inner: Arc<Mutex<Connection>>,
    #[allow(dead_code)]
    path: PathBuf,
}

impl Db {
    pub fn open(path: &Path) -> PicOrgResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "cache_size", -64_000)?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;

        migrations::run(&conn)?;

        Ok(Db {
            inner: Arc::new(Mutex::new(conn)),
            path: path.to_path_buf(),
        })
    }

    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> PicOrgResult<T>) -> PicOrgResult<T> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| PicOrgError::Pool("db mutex poisoned".into()))?;
        f(&guard)
    }

    pub fn with_conn_mut<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> PicOrgResult<T>,
    ) -> PicOrgResult<T> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| PicOrgError::Pool("db mutex poisoned".into()))?;
        f(&mut guard)
    }
}
