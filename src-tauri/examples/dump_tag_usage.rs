//! Prints how many images (and which) are tagged with the tag named in
//! the `MAGPIE_QUERY_TAG` env var, across every registered per-folder
//! library. Used by `scripts/test-multiselect-tag.ps1` to verify that a
//! batch tag save actually landed in the DB.

fn main() {
    let tag = std::env::var("MAGPIE_QUERY_TAG")
        .or_else(|_| std::env::var("PICORG_QUERY_TAG"))
        .unwrap_or_default();
    let reg_path = dirs::data_dir()
        .expect("data dir")
        .join("com.magpie.app")
        .join("registry.db");
    println!("Registry DB: {}", reg_path.display());
    if tag.is_empty() {
        println!("(no MAGPIE_QUERY_TAG set)");
        return;
    }
    if !reg_path.exists() {
        println!("(registry.db missing — launch Magpie once first)");
        return;
    }

    let reg = rusqlite::Connection::open(&reg_path).expect("open registry");
    let mut stmt = reg
        .prepare("SELECT id, path FROM library_folders")
        .expect("prep");
    let folders: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .expect("query")
        .flatten()
        .collect();

    let mut total = 0i64;
    for (fid, path) in folders {
        let db_path = std::path::PathBuf::from(&path)
            .join(".magpie")
            .join("library.db");
        if !db_path.exists() {
            println!("---- folder {fid} @ {path} : library missing ----");
            continue;
        }
        let lib = rusqlite::Connection::open(&db_path).expect("open library");
        let count: i64 = lib
            .query_row(
                "SELECT COUNT(*)
                 FROM image_tags it
                 JOIN tags t ON t.id = it.tag_id
                 WHERE t.name = ?1 COLLATE NOCASE",
                [&tag],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if count == 0 {
            continue;
        }
        total += count;
        println!("---- folder {fid} @ {path} : {count} tagged ----");

        let mut stmt = lib
            .prepare(
                "SELECT i.id, i.rel_path
                 FROM images i
                 JOIN image_tags it ON it.image_id = i.id
                 JOIN tags t ON t.id = it.tag_id
                 WHERE t.name = ?1 COLLATE NOCASE
                 ORDER BY i.id",
            )
            .expect("prep");
        let rows = stmt
            .query_map([&tag], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .expect("query");
        for r in rows.flatten() {
            println!("  id={} rel={}", r.0, r.1);
        }
    }
    println!("Tag '{tag}' is applied to {total} images total");
}
