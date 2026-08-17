//! Diagnostic tool: dumps everything we know about an image on disk.
//!
//! Usage:
//!     cargo run -q --example dump_meta -- "C:\path\to\photo.jpg"
//!
//! Or, without a path, prints the first few rows of the release DB and
//! runs read_all on each.

use picorg_lib::core::metadata::read as meta_read;
use picorg_lib::core::metadata::sidecar::sidecar_path_for;
use picorg_lib::core::metadata::xmp;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        dump_first_rows_from_db();
    } else {
        for a in args {
            dump_one(&PathBuf::from(a));
        }
    }
}

fn dump_one(path: &std::path::Path) {
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

    let sidecar = sidecar_path_for(path);
    println!("Sidecar path: {}", sidecar.display());
    println!("  sidecar exists = {}", sidecar.exists());
    if sidecar.exists() {
        match std::fs::read_to_string(&sidecar) {
            Ok(s) => println!("  sidecar preview:\n{}", indent(&s, "    ")),
            Err(e) => println!("  sidecar read err: {}", e),
        }
    }

    match xmp::extract_embedded_xmp(path) {
        Ok(Some(bytes)) => {
            println!("Embedded XMP: {} bytes", bytes.len());
            let s = String::from_utf8_lossy(&bytes);
            println!("{}", indent(&s, "    "));
            match xmp::parse_user_metadata(&bytes) {
                Ok(m) => println!("  parsed embedded XMP: {:?}", m),
                Err(e) => println!("  parse err: {}", e),
            }
        }
        Ok(None) => println!("Embedded XMP: (none)"),
        Err(e) => println!("Embedded XMP read err: {}", e),
    }

    println!("---- meta_read::read_all ----");
    match meta_read::read_all(path) {
        Ok(m) => {
            println!("  title      = {:?}", m.title);
            println!("  comment    = {:?}", m.comment);
            println!("  rating     = {:?}", m.rating);
            println!("  tags       = {:?}", m.tags);
            println!("  taken_at   = {:?}", m.taken_at);
            println!("  camera     = {:?} {:?}", m.camera_make, m.camera_model);
            println!("  width/height = {:?} x {:?}", m.width, m.height);
        }
        Err(e) => println!("  read_all err: {}", e),
    }
}

fn dump_first_rows_from_db() {
    let db_path = dirs::data_dir()
        .expect("data dir")
        .join("com.picorg.picorg")
        .join("picorg.db");
    println!("DB path: {}", db_path.display());
    println!("  exists = {}", db_path.exists());
    if !db_path.exists() {
        return;
    }
    let conn = rusqlite::Connection::open(&db_path).expect("open DB");
    let mut stmt = conn
        .prepare(
            "SELECT id, path, title, rating,
                    (SELECT GROUP_CONCAT(t.name, ',')
                     FROM tags t JOIN image_tags it ON it.tag_id = t.id
                     WHERE it.image_id = images.id) AS tags,
                    meta_read_at, meta_written_at
             FROM images
             WHERE (SELECT COUNT(*) FROM image_tags it WHERE it.image_id = images.id) > 0
                OR meta_read_at IS NOT NULL
                OR id IN (5, 12)
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
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })
        .expect("query");
    for r in rows.flatten() {
        println!("id={}  path={}  title={:?}  rating={:?}  tags={:?}", r.0, r.1, r.2, r.3, r.4);
        println!("     meta_read_at={:?}  meta_written_at={:?}", r.5, r.6);
        dump_one(std::path::Path::new(&r.1));
    }
}

fn indent(s: &str, prefix: &str) -> String {
    s.lines()
        .map(|l| format!("{}{}", prefix, l))
        .collect::<Vec<_>>()
        .join("\n")
}
