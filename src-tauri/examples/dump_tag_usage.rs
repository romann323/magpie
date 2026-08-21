//! Prints how many images (and which) are tagged with the tag named in
//! the `MAGPIE_QUERY_TAG` env var. Used by `scripts/test-multiselect-tag.ps1`
//! to verify that a batch tag save actually landed in the DB.

fn main() {
    // Old name kept as a fallback for compatibility with older invocations.
    let tag = std::env::var("MAGPIE_QUERY_TAG")
        .or_else(|_| std::env::var("PICORG_QUERY_TAG"))
        .unwrap_or_default();
    let db_path = dirs::data_dir()
        .expect("data dir")
        .join("com.magpie.app")
        .join("library.db");
    println!("DB: {}", db_path.display());
    if tag.is_empty() {
        println!("(no MAGPIE_QUERY_TAG set)");
        return;
    }
    let conn = rusqlite::Connection::open(&db_path).expect("open DB");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM image_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE t.name = ?1 COLLATE NOCASE",
            [&tag],
            |r| r.get(0),
        )
        .unwrap_or(-1);
    println!("Tag '{tag}' is applied to {count} images");

    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.path
             FROM images i
             JOIN image_tags it ON it.image_id = i.id
             JOIN tags t ON t.id = it.tag_id
             WHERE t.name = ?1 COLLATE NOCASE
             ORDER BY i.id",
        )
        .expect("prep");
    let rows = stmt
        .query_map([&tag], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .expect("query");
    for r in rows.flatten() {
        println!("  id={} path={}", r.0, r.1);

        let sidecar = std::path::PathBuf::from(&r.1).with_extension("xmp");
        if sidecar.exists() {
            let body = std::fs::read_to_string(&sidecar).unwrap_or_default();
            let contains = body.contains(&tag);
            println!(
                "    sidecar exists ({} bytes), contains tag = {}",
                body.len(),
                contains
            );
        } else {
            println!("    sidecar missing at {}", sidecar.display());
        }
    }
}
