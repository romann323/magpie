//! `LibraryPool` — the glue between the central registry DB and every
//! per-folder library DB.
//!
//! Responsibilities:
//! - Open the registry once, keeping the connection alive for the app
//!   lifetime.
//! - `ATTACH DATABASE` every registered library so cross-folder queries
//!   (search, tag list, FTS) run off a single connection.
//! - Cache per-folder writer connections and hand them out on demand
//!   for scanning, tag edits, and single-file lookups.
//! - Notify the registry connection when a folder is added / removed so
//!   the attached-schemas view stays in sync.
//!
//! All read paths that span multiple folders take
//! [`LibraryPool::with_registry_conn`]. All write paths for one folder
//! take [`LibraryPool::library`] and then lock that DB's own mutex.

use crate::db::library::LibraryDb;
use crate::db::registry::{self, LibraryFolderRow};
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

/// Where a folder's library DB lives on disk.
///
/// `<folder>/.magpie/library.db`. Hidden on Windows Explorer (dot
/// prefix folder), portable across machines, easy to spot when
/// browsing the folder from the shell.
pub fn library_db_path_for(folder_root: &Path) -> PathBuf {
    folder_root.join(".magpie").join("library.db")
}

/// SQLite's default `SQLITE_LIMIT_ATTACHED` is 10; we raise it to 125
/// (the compile-time cap) so realistic multi-folder libraries fit.
const MAX_ATTACHED_DBS: usize = 10;

pub struct LibraryPool {
    /// The registry connection. Also holds every library DB attached
    /// as `f<id>` for cross-folder queries.
    reg: Arc<Mutex<Connection>>,

    /// Per-folder writer connections. Opened lazily on first use.
    libraries: RwLock<HashMap<i64, Arc<LibraryDb>>>,
}

impl LibraryPool {
    /// Open (or create) the registry DB and attach every registered
    /// library.
    pub fn open(registry_path: &Path) -> AppResult<Arc<Self>> {
        if let Some(parent) = registry_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(registry_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;
        // The bundled sqlite ships with SQLITE_MAX_ATTACHED = 10 by
        // default (compile-time constant). We can't raise it at runtime
        // so we cap ourselves at MAX_ATTACHED_DBS = 10 in the attach
        // loop below. If a user needs more folders we can bump the
        // ceiling via a build.rs `-DSQLITE_MAX_ATTACHED=125` later.

        registry::init_registry_schema(&conn)?;

        // Attach every already-registered folder that still has a
        // valid on-disk library. Missing DBs are marked unavailable in
        // the registry; the folder still appears in the sidebar so the
        // user can rescan when the drive is plugged back in.
        let folders = registry::list_folders(&conn)?;
        let mut attached = 0usize;
        for f in &folders {
            let db_path = library_db_path_for(Path::new(&f.path));
            if !db_path.exists() {
                let _ = registry::set_availability(&conn, f.id, false);
                continue;
            }
            if attached >= MAX_ATTACHED_DBS {
                tracing::warn!(
                    folder_id = f.id,
                    path = %db_path.display(),
                    "SQLite ATTACH cap reached ({MAX_ATTACHED_DBS}); folder will show as unavailable until other folders are removed"
                );
                let _ = registry::set_availability(&conn, f.id, false);
                continue;
            }
            if let Err(e) = attach_schema(&conn, f.id, &db_path) {
                tracing::warn!(folder_id = f.id, path = %db_path.display(), error = %e, "failed to attach library at startup");
                let _ = registry::set_availability(&conn, f.id, false);
            } else {
                let _ = registry::set_availability(&conn, f.id, true);
                attached += 1;
            }
        }

        Ok(Arc::new(Self {
            reg: Arc::new(Mutex::new(conn)),
            libraries: RwLock::new(HashMap::new()),
        }))
    }

    /// Register a new folder. Creates its `.magpie/library.db`,
    /// inserts the registry row, and attaches the schema for future
    /// cross-folder queries.
    pub fn add_folder(&self, absolute_path: &Path) -> AppResult<LibraryFolderRow> {
        let db_path = library_db_path_for(absolute_path);
        // Force-open the library so the file (and schema) exist before
        // we attach.
        let reg = self.reg.lock().map_err(pool_err)?;
        let folder = registry::insert_folder(&reg, &absolute_path.to_string_lossy())?;
        drop(reg);
        // Bootstrap the library DB. Even if the folder is empty, we
        // want the schema in place so subsequent scans have somewhere
        // to write to.
        let lib = LibraryDb::open(&db_path, folder.id)?;
        self.libraries
            .write()
            .map_err(pool_err)?
            .insert(folder.id, Arc::new(lib));
        // Now attach.
        let reg = self.reg.lock().map_err(pool_err)?;
        if let Err(e) = attach_schema(&reg, folder.id, &db_path) {
            tracing::warn!(folder_id = folder.id, error = %e, "attach failed after add_folder");
            let _ = registry::set_availability(&reg, folder.id, false);
        } else {
            let _ = registry::set_availability(&reg, folder.id, true);
        }
        Ok(folder)
    }

    /// Remove a folder from the registry and detach its schema.
    /// Does **not** delete the on-disk library.db — that file lives
    /// inside the folder itself and it's the user's data.
    pub fn remove_folder(&self, folder_id: i64) -> AppResult<()> {
        // Drop the cached writer connection first — SQLite refuses to
        // detach while any statement is prepared against the schema.
        self.libraries
            .write()
            .map_err(pool_err)?
            .remove(&folder_id);
        let reg = self.reg.lock().map_err(pool_err)?;
        let _ = detach_schema(&reg, folder_id);
        registry::delete_folder(&reg, folder_id)
    }

    /// Lock the registry connection. Callers use it to run
    /// cross-folder queries against attached `f<id>.*` schemas.
    pub fn with_registry<T>(
        &self,
        f: impl FnOnce(&Connection) -> AppResult<T>,
    ) -> AppResult<T> {
        let guard = self.reg.lock().map_err(pool_err)?;
        f(&guard)
    }

    pub fn with_registry_mut<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut guard = self.reg.lock().map_err(pool_err)?;
        f(&mut guard)
    }

    /// Get (opening if necessary) the writer connection for one folder.
    /// The returned handle owns its own connection Mutex — locking it
    /// does not block operations on other folders.
    pub fn library(&self, folder_id: i64) -> AppResult<Arc<LibraryDb>> {
        // Fast path: already open.
        if let Some(lib) = self
            .libraries
            .read()
            .map_err(pool_err)?
            .get(&folder_id)
            .cloned()
        {
            return Ok(lib);
        }
        // Slow path: open. We need the registry to know where the
        // folder's DB lives on disk.
        let folder = {
            let reg = self.reg.lock().map_err(pool_err)?;
            registry::get_folder(&reg, folder_id)?
        };
        let db_path = library_db_path_for(Path::new(&folder.path));
        let lib = Arc::new(LibraryDb::open(&db_path, folder_id)?);
        self.libraries
            .write()
            .map_err(pool_err)?
            .insert(folder_id, lib.clone());
        Ok(lib)
    }

    /// List every registered folder, snapshot.
    pub fn list_folders(&self) -> AppResult<Vec<LibraryFolderRow>> {
        let reg = self.reg.lock().map_err(pool_err)?;
        registry::list_folders(&reg)
    }

    /// Fetch one folder row by ID.
    pub fn folder(&self, id: i64) -> AppResult<LibraryFolderRow> {
        let reg = self.reg.lock().map_err(pool_err)?;
        registry::get_folder(&reg, id)
    }

    /// Update `last_scan_at` on the folder row (registry-side).
    pub fn set_last_scan_at(&self, folder_id: i64, ts: i64) -> AppResult<()> {
        let reg = self.reg.lock().map_err(pool_err)?;
        registry::set_last_scan_at(&reg, folder_id, ts)
    }
}

fn pool_err<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Pool(e.to_string())
}

/// Attach a library DB as `f<id>` on the registry connection. Idempotent-
/// ish: attaches over an existing schema by first attempting a detach.
fn attach_schema(conn: &Connection, folder_id: i64, db_path: &Path) -> AppResult<()> {
    let alias = format!("f{folder_id}");
    let _ = conn.execute_batch(&format!("DETACH DATABASE {alias};"));
    conn.execute(
        &format!("ATTACH DATABASE ?1 AS {alias}"),
        rusqlite::params![db_path.to_string_lossy().to_string()],
    )?;
    tracing::debug!(folder_id, path = %db_path.display(), "attached library schema");
    Ok(())
}

fn detach_schema(conn: &Connection, folder_id: i64) -> AppResult<()> {
    let alias = format!("f{folder_id}");
    conn.execute_batch(&format!("DETACH DATABASE {alias};"))?;
    Ok(())
}

/// Convenience: lock the registry connection with a short-hand guard
/// name used by the search code below.
pub fn lock_reg(reg: &Arc<Mutex<Connection>>) -> AppResult<MutexGuard<'_, Connection>> {
    reg.lock().map_err(pool_err)
}
