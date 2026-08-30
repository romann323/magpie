//! End-to-end tests for the metadata **read** pipeline and the central
//! Magpie DB. Magpie doesn't write back into source files, so the old
//! roundtrip suite is gone; what remains covers:
//!
//! - Legacy `.xmp` sidecar reads (Lightroom / old-Magpie compat).
//! - Registry recognises every expected extension for the scanner.
//! - `db::queries` upsert + apply_metadata_patch round-trip on a real
//!   on-disk `magpie.db`.

use desktop_lib::core::formats::FormatRegistry;
use desktop_lib::core::metadata::read as meta_read;
use desktop_lib::db::queries::{self, FileStat, ImageMetaFromFile, MetadataPatch};
use desktop_lib::db::Db;
use std::io::Write;
use std::path::Path;

fn write_sidecar(image_path: &std::path::Path, xml: &str) {
    let sidecar = image_path.with_extension("xmp");
    let mut f = std::fs::File::create(sidecar).unwrap();
    f.write_all(xml.as_bytes()).unwrap();
}

fn registry() -> FormatRegistry {
    FormatRegistry::new()
}

// ---------- Legacy sidecar read ----------

#[test]
fn read_sidecar_end_to_end() {
    let tmp = tempdir();
    let img = tmp.join("photo.png");
    std::fs::write(&img, tiny_png()).unwrap();

    write_sidecar(
        &img,
        r#"<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:xmp="http://ns.adobe.com/xap/1.0/"
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmp:Rating="4">
      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Alpha Sunset</rdf:li>
        </rdf:Alt>
      </dc:title>
      <dc:subject>
        <rdf:Bag>
          <rdf:li>vacation</rdf:li>
          <rdf:li>sunset</rdf:li>
          <rdf:li>2024</rdf:li>
        </rdf:Bag>
      </dc:subject>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#,
    );

    let meta = meta_read::read_all(&registry(), &img).expect("read_all failed");
    assert_eq!(meta.title.as_deref(), Some("Alpha Sunset"));
    assert_eq!(
        meta.tags,
        vec!["vacation".to_string(), "sunset".to_string(), "2024".to_string()]
    );
}

#[test]
fn read_sidecar_case_variants() {
    let tmp = tempdir();
    let img = tmp.join("photo2.png");
    std::fs::write(&img, tiny_png()).unwrap();
    write_sidecar(
        &img,
        r#"<?xml version="1.0"?>
<X:XMPMETA xmlns:X="adobe:ns:meta/">
  <RDF:RDF xmlns:RDF="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <RDF:Description RDF:about=""
        xmlns:DC="http://purl.org/dc/elements/1.1/">
      <DC:subject>
        <RDF:Bag>
          <RDF:li>UPPERCASE</RDF:li>
        </RDF:Bag>
      </DC:subject>
    </RDF:Description>
  </RDF:RDF>
</X:XMPMETA>"#,
    );
    let meta = meta_read::read_all(&registry(), &img).unwrap();
    assert_eq!(meta.tags, vec!["UPPERCASE".to_string()]);
}

// ---------- Registry sanity ----------

#[test]
fn registry_recognises_every_expected_extension() {
    let r = registry();
    for ext in [
        "jpg", "jpeg", "png", "webp", "gif", "tif", "tiff", "dng", "heic", "heif", "avif", "jxl",
        "psd", "pdf", "mp4", "mov", "mkv", "webm", "avi", "cr2", "nef", "arw", "raf", "bmp", "svg",
    ] {
        assert!(
            r.for_ext(ext).is_some(),
            "expected registry to answer for `.{ext}` — got None"
        );
    }
}

// ---------- Central DB round-trip ----------

/// Open a fresh magpie.db, register a folder, upsert an image, patch
/// its metadata, then confirm title + tags round-trip out of the DB.
#[test]
fn central_db_upsert_and_patch_roundtrip() {
    let tmp = tempdir();
    let db_path = tmp.join("magpie.db");
    let db = Db::open(&db_path).expect("open db");

    let folder_root = tmp.join("photos");
    std::fs::create_dir_all(&folder_root).unwrap();
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, &folder_root))
        .expect("insert folder");

    let stat = FileStat {
        folder_id: folder.id,
        rel_path: "photos/a.jpg".to_string(),
        filename: "a.jpg",
        ext: "jpg",
        size_bytes: 123,
        mtime_ms: 42,
    };
    let outcome = db
        .with_conn(|conn| queries::upsert_image(conn, &stat))
        .expect("upsert");
    let image_id = outcome.id();

    let patch = MetadataPatch {
        title: Some(Some("Alpha".into())),
        tags: Some(vec!["vacation".into(), "sunset".into()]),
        tags_add: None,
        tags_remove: None,
    };
    db.with_conn_mut(|conn| queries::apply_metadata_patch(conn, image_id, &patch))
        .expect("apply patch");

    let row = db
        .with_conn(|conn| queries::get_image_row(conn, image_id))
        .unwrap()
        .unwrap();
    assert_eq!(row.title.as_deref(), Some("Alpha"));
    // `MetadataPatch.tags` writes into the user source.
    let mut user_tags = row.user_tags.clone();
    user_tags.sort();
    assert_eq!(
        user_tags,
        vec!["sunset".to_string(), "vacation".to_string()]
    );
    assert!(row.auto_tags.is_empty());
    assert_eq!(row.folder_id, folder.id);

    // Removing the folder should cascade — image + tag join rows go.
    db.with_conn(|conn| queries::delete_folder(conn, folder.id))
        .unwrap();
    assert!(db
        .with_conn(|conn| queries::get_image_row(conn, image_id))
        .unwrap()
        .is_none());
}

/// Renaming an image updates `filename`, `rel_path`, `ext`, and the
/// FTS row that indexes the old filename.
#[test]
fn rename_image_row_updates_filename_and_fts() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, Path::new(&tmp)))
        .unwrap();
    let stat = FileStat {
        folder_id: folder.id,
        rel_path: "sub/old.jpg".to_string(),
        filename: "old.jpg",
        ext: "jpg",
        size_bytes: 1,
        mtime_ms: 1,
    };
    let id = db
        .with_conn(|conn| queries::upsert_image(conn, &stat))
        .unwrap()
        .id();
    db.with_conn_mut(|conn| queries::rename_image_row(conn, id, "new.png"))
        .expect("rename");
    let row = db
        .with_conn(|conn| queries::get_image_row(conn, id))
        .unwrap()
        .unwrap();
    assert_eq!(row.filename, "new.png");
    assert_eq!(row.rel_path, "sub/new.png");
    assert_eq!(row.ext, "png");
}

/// Renaming a second file into a slot already used by another must
/// fail without touching the DB.
#[test]
fn rename_image_row_rejects_collisions() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, Path::new(&tmp)))
        .unwrap();
    for name in ["a.jpg", "b.jpg"] {
        let stat = FileStat {
            folder_id: folder.id,
            rel_path: name.to_string(),
            filename: name,
            ext: "jpg",
            size_bytes: 1,
            mtime_ms: 1,
        };
        db.with_conn(|conn| queries::upsert_image(conn, &stat))
            .unwrap();
    }
    let b_id: i64 = db
        .with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id FROM images WHERE rel_path = 'b.jpg'",
                    [],
                    |r| r.get(0),
                )
                .unwrap())
        })
        .unwrap();
    let err = db
        .with_conn_mut(|conn| queries::rename_image_row(conn, b_id, "a.jpg"))
        .unwrap_err();
    assert!(matches!(err, desktop_lib::error::AppError::BadInput(_)));
}

/// The scanner path (`set_image_meta`) writes automatic tags without
/// touching the user's typed tags, and never removes anything a
/// previous scan added. This is the whole point of the `source` split.
#[test]
fn scanner_never_wipes_user_tags() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, Path::new(&tmp)))
        .unwrap();
    let stat = FileStat {
        folder_id: folder.id,
        rel_path: "a.jpg".to_string(),
        filename: "a.jpg",
        ext: "jpg",
        size_bytes: 1,
        mtime_ms: 1,
    };
    let id = db
        .with_conn(|conn| queries::upsert_image(conn, &stat))
        .unwrap()
        .id();

    // 1) First scan writes ["vacation", "sunset"] from the file itself.
    let scan1 = ImageMetaFromFile {
        tags: vec!["vacation".into(), "sunset".into()],
        ..Default::default()
    };
    db.with_conn_mut(|conn| queries::set_image_meta(conn, id, &scan1))
        .unwrap();

    // 2) The user adds "keeper" via the UI and manually types "sunset"
    //    too — that name is now recorded as *both* auto and user.
    db.with_conn_mut(|conn| {
        queries::apply_metadata_patch(
            conn,
            id,
            &MetadataPatch {
                title: None,
                tags: Some(vec!["keeper".into(), "sunset".into()]),
                tags_add: None,
                tags_remove: None,
            },
        )
    })
    .unwrap();

    // 3) The user re-edits the file outside Magpie; a rescan now sees
    //    only ["vacation", "family"] embedded in the file.
    let scan2 = ImageMetaFromFile {
        tags: vec!["vacation".into(), "family".into()],
        ..Default::default()
    };
    db.with_conn_mut(|conn| queries::set_image_meta(conn, id, &scan2))
        .unwrap();

    let row = db
        .with_conn(|conn| queries::get_image_row(conn, id))
        .unwrap()
        .unwrap();

    let mut user = row.user_tags.clone();
    user.sort();
    assert_eq!(
        user,
        vec!["keeper".to_string(), "sunset".to_string()],
        "user tags must survive rescans"
    );

    let mut auto = row.auto_tags.clone();
    auto.sort();
    // `sunset` originally came from auto and is still auto after both
    // rescans (the second scan doesn't list it any more, but we never
    // delete auto rows). `family` was added by the second scan.
    // `vacation` was already in auto and doesn't get duplicated.
    assert_eq!(
        auto,
        vec![
            "family".to_string(),
            "sunset".to_string(),
            "vacation".to_string()
        ]
    );

    // Aggregated view: everything unique.
    let mut all = db
        .with_conn(|conn| queries::tags_for_image(conn, id))
        .unwrap();
    all.sort();
    assert_eq!(
        all,
        vec![
            "family".to_string(),
            "keeper".to_string(),
            "sunset".to_string(),
            "vacation".to_string()
        ]
    );
}

/// UI remove targets `'user'` only; an auto row with the same name
/// stays put and the tag is still visible via the aggregated view.
#[test]
fn user_remove_leaves_auto_row_intact() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, Path::new(&tmp)))
        .unwrap();
    let stat = FileStat {
        folder_id: folder.id,
        rel_path: "a.jpg".to_string(),
        filename: "a.jpg",
        ext: "jpg",
        size_bytes: 1,
        mtime_ms: 1,
    };
    let id = db
        .with_conn(|conn| queries::upsert_image(conn, &stat))
        .unwrap()
        .id();
    let scan = ImageMetaFromFile {
        tags: vec!["beach".into()],
        ..Default::default()
    };
    db.with_conn_mut(|conn| queries::set_image_meta(conn, id, &scan))
        .unwrap();
    db.with_conn_mut(|conn| {
        queries::apply_metadata_patch(
            conn,
            id,
            &MetadataPatch {
                title: None,
                tags: None,
                tags_add: Some(vec!["beach".into()]),
                tags_remove: None,
            },
        )
    })
    .unwrap();
    // Now: auto={beach}, user={beach}. Removing it from user…
    db.with_conn_mut(|conn| {
        queries::apply_metadata_patch(
            conn,
            id,
            &MetadataPatch {
                title: None,
                tags: None,
                tags_add: None,
                tags_remove: Some(vec!["beach".into()]),
            },
        )
    })
    .unwrap();
    let row = db
        .with_conn(|conn| queries::get_image_row(conn, id))
        .unwrap()
        .unwrap();
    assert!(row.user_tags.is_empty(), "user side cleared");
    assert_eq!(row.auto_tags, vec!["beach".to_string()], "auto stays");
    let all = db
        .with_conn(|conn| queries::tags_for_image(conn, id))
        .unwrap();
    assert_eq!(all, vec!["beach".to_string()], "aggregated still sees it");
}

/// FTS survives a rename + delete cycle on a shared tag.
#[test]
fn tag_rename_and_delete_update_fts() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, Path::new(&tmp)))
        .unwrap();
    let stat = FileStat {
        folder_id: folder.id,
        rel_path: "a.jpg".to_string(),
        filename: "a.jpg",
        ext: "jpg",
        size_bytes: 1,
        mtime_ms: 1,
    };
    let id = db
        .with_conn(|conn| queries::upsert_image(conn, &stat))
        .unwrap()
        .id();
    db.with_conn_mut(|conn| {
        queries::apply_metadata_patch(
            conn,
            id,
            &MetadataPatch {
                title: None,
                tags: Some(vec!["beach".into()]),
                tags_add: None,
                tags_remove: None,
            },
        )
    })
    .unwrap();
    db.with_conn_mut(|conn| queries::rename_tag(conn, "beach", "coast"))
        .unwrap();
    let tags = db
        .with_conn(|conn| queries::tags_for_image(conn, id))
        .unwrap();
    assert_eq!(tags, vec!["coast".to_string()]);
    db.with_conn_mut(|conn| queries::delete_tag(conn, "coast"))
        .unwrap();
    let tags = db
        .with_conn(|conn| queries::tags_for_image(conn, id))
        .unwrap();
    assert!(tags.is_empty());
}

// ---------- Fixtures ----------

fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x3B,
        0x7E, 0x9B, 0x55, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

fn tempdir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join("magpie_test")
        .join(format!("t{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
