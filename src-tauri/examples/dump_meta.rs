//! Diagnostic tool: dumps every metadata field the format-handler registry
//! can extract from a file.
//!
//! Usage:
//!     cargo run -q --example dump_meta -- "C:\path\to\photo.jpg"
//!
//! With no arguments, prints the first few rows of the release DB and runs
//! the registry on each.

use desktop_lib::core::formats::FormatRegistry;
use desktop_lib::core::metadata::read as meta_read;
use desktop_lib::core::metadata::sidecar::sidecar_path_for;
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
            "Handler: name={}  can_write_tags={}  extensions={:?}",
            h.name(),
            h.can_write_tags(),
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
        .join("library.db");
    println!("DB path: {}", db_path.display());
    println!("  exists = {}", db_path.exists());
    if !db_path.exists() {
        return;
    }
    let conn = rusqlite::Connection::open(&db_path).expect("open DB");
    let mut stmt = conn
        .prepare(
            "SELECT id, path, title,
                    (SELECT GROUP_CONCAT(t.name, ',')
                     FROM tags t JOIN image_tags it ON it.tag_id = t.id
                     WHERE it.image_id = images.id) AS tags,
                    meta_read_at, meta_written_at
             FROM images
             ORDER BY id ASC
             LIMIT 20",
        )
        .expect("prepare");
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })
        .expect("query");
    for r in rows.flatten() {
        println!("id={}  path={}  title={:?}  tags={:?}", r.0, r.1, r.2, r.3);
        println!("     meta_read_at={:?}  meta_written_at={:?}", r.4, r.5);
        dump_one(registry, std::path::Path::new(&r.1));
    }
}
