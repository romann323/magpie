//! Prints how many images (and which) are tagged with the tag named in
//! the `MAGPIE_QUERY_TAG` env var. Used by
//! `scripts/test-multiselect-tag.ps1` to verify that a batch tag save
//! actually landed in the DB.

fn main() {
    let tag = std::env::var("MAGPIE_QUERY_TAG")
        .or_else(|_| std::env::var("PICORG_QUERY_TAG"))
        .unwrap_or_default();
    let db_path = dirs::data_dir()
        .expect("data dir")
        .join("com.magpie.app")
        .join(desktop_lib::db::DB_FILE_NAME);
    println!("Magpie DB: {}", db_path.display());
    if tag.is_empty() {
        println!("(no MAGPIE_QUERY_TAG set)");
        return;
    }
    if !db_path.exists() {
        println!("(magpie.db missing — launch Magpie once first)");
        return;
    }

    let db = rusqlite::Connection::open(&db_path).expect("open db");
    let total: i64 = db
        .query_row(
            "SELECT COUNT(*)
             FROM image_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE t.name = ?1 COLLATE NOCASE",
            [&tag],
            |r| r.get(0),
        )
        .unwrap_or(0);
    println!("Tag '{tag}' is applied to {total} images total\n");
    if total == 0 {
        return;
    }

    let mut stmt = db
        .prepare(
            "SELECT i.id, f.path, i.rel_path
             FROM images i
             JOIN image_tags it ON it.image_id = i.id
             JOIN tags t ON t.id = it.tag_id
             JOIN library_folders f ON f.id = i.folder_id
             WHERE t.name = ?1 COLLATE NOCASE
             ORDER BY f.id, i.id",
        )
        .expect("prep");
    let rows = stmt
        .query_map([&tag], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .expect("query");
    for r in rows.flatten() {
        println!("  id={}  {}", r.0, std::path::PathBuf::from(&r.1).join(&r.2).display());
    }
}
