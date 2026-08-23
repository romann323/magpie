//! Diagnostic tool: dumps every metadata field the format-handler
//! registry can extract from a file. Read-only — Magpie never writes
//! back into source files.
//!
//! Usage:
//!     cargo run -q --example dump_meta -- "C:\path\to\photo.jpg"
//!
//! With no arguments, prints the first few rows from magpie.db
//! (assuming Magpie has been launched at least once so the DB exists)
//! and runs the registry on each row's file.

use desktop_lib::core::formats::FormatRegistry;
use desktop_lib::core::metadata::read as meta_read;
use desktop_lib::core::metadata::sidecar::sidecar_path_for;
use desktop_lib::db;
use std::path::PathBuf;

fn main() {
    let registry = FormatRegistry::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        dump_first_rows_from_db(&registry);
    } else {
        for a in args {
            dump_one(&registry, &PathBuf::from(a));
        }
    }
}

fn dump_one(registry: &FormatRegistry, path: &std::path::Path) {
    println!("=====================================================");
    println!("File: {}", path.display());
    println!(
        "exists = {}, canonicalize = {:?}",
        path.exists(),
        std::fs::canonicalize(path).ok()
    );
    match std::fs::metadata(path) {
        Ok(m) => println!(
            "  size = {} bytes, mtime = {:?}, readonly = {}",
            m.len(),
            m.modified().ok(),
            m.permissions().readonly()
        ),
        Err(e) => println!("  metadata error: {}", e),
    }

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let handler = registry.for_ext(ext);
    match handler {
        Some(h) => println!(
            "Handler: name={}  extensions={:?}",
            h.name(),
            h.extensions()
        ),
        None => println!("Handler: (none — unrecognised extension `{}`)", ext),
    }

    let sidecar = sidecar_path_for(path);
    println!("Legacy sidecar path: {}  exists={}", sidecar.display(), sidecar.exists());

    if let Some(h) = handler {
        println!("---- read_technical ----");
        for (k, v) in &h.read_technical(path).entries {
            println!("  {:<20} = {}", k, v);
        }
        println!("---- read_user ----");
        match h.read_user(path) {
            Ok(u) => {
                println!("  title = {:?}", u.title);
                println!("  tags  = {:?}", u.tags);
            }
            Err(e) => println!("  err: {}", e),
        }
    }

    println!("---- meta_read::read_all (DB row) ----");
    match meta_read::read_all(registry, path) {
        Ok(m) => {
            println!("  title      = {:?}", m.title);
            println!("  tags       = {:?}", m.tags);
            println!("  taken_at   = {:?}", m.taken_at);
            println!("  camera     = {:?} {:?}", m.camera_make, m.camera_model);
            println!("  dimensions = {:?} x {:?}", m.width, m.height);
        }
        Err(e) => println!("  read_all err: {}", e),
    }
}

fn dump_first_rows_from_db(registry: &FormatRegistry) {
    let db_path = dirs::data_dir()
        .expect("data dir")
        .join("com.magpie.app")
        .join(db::DB_FILE_NAME);
    println!("Magpie DB path: {}", db_path.display());
    println!("  exists = {}", db_path.exists());
    if !db_path.exists() {
        return;
    }

    let db = db::Db::open(&db_path).expect("open db");
    let folders = db.with_conn(db::queries::list_folders).expect("folders");

    for f in folders {
        println!("---- folder {} @ {} ----", f.id, f.path);
        let rows = db
            .with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, rel_path, title,
                            (SELECT GROUP_CONCAT(t.name, ',')
                             FROM tags t JOIN image_tags it ON it.tag_id = t.id
                             WHERE it.image_id = images.id) AS tags
                     FROM images
                     WHERE folder_id = ?1
                     ORDER BY id ASC
                     LIMIT 5",
                )?;
                let rows = stmt.query_map(rusqlite::params![f.id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?;
                Ok(rows.collect::<Result<Vec<_>, _>>()?)
            })
            .expect("query images");

        for r in rows {
            let abs = std::path::PathBuf::from(&f.path).join(&r.1);
            println!(
                "  id={}  rel={}  title={:?}  tags={:?}",
                r.0, r.1, r.2, r.3
            );
            dump_one(registry, &abs);
        }
    }
}
