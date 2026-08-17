//! End-to-end test for the "load metadata from FS on selection" behaviour.
//!
//! Verifies that when a JPEG sidecar exists on disk with tags, title, rating
//! and description, our reader parses all of them out. This is the code path
//! that `get_image` uses when it detects that the FS is newer than the DB.

use picorg_lib::core::metadata::read as meta_read;
use std::io::Write;

fn write_sidecar(image_path: &std::path::Path, xml: &str) {
    let sidecar = image_path.with_extension("xmp");
    let mut f = std::fs::File::create(sidecar).unwrap();
    f.write_all(xml.as_bytes()).unwrap();
}

#[test]
fn read_sidecar_end_to_end() {
    let tmp = tempdir();
    let img = tmp.join("photo.png");

    // We only care about sidecar parsing, not image decoding, so any bytes
    // are fine.
    std::fs::write(&img, b"placeholder").unwrap();

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
      <dc:description>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">A test description</rdf:li>
        </rdf:Alt>
      </dc:description>
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

    let meta = meta_read::read_all(&img).expect("read_all failed");
    assert_eq!(meta.title.as_deref(), Some("Alpha Sunset"));
    assert_eq!(meta.comment.as_deref(), Some("A test description"));
    assert_eq!(meta.rating, Some(4));
    assert_eq!(
        meta.tags,
        vec!["vacation".to_string(), "sunset".to_string(), "2024".to_string()]
    );
}

/// Windows Explorer tags: dc:subject is written into the file's embedded XMP,
/// but we don't have a real JPEG here — so simulate via sidecar (same code path
/// used by both). This just double-checks the case-insensitive namespace handling.
#[test]
fn read_sidecar_case_variants() {
    let tmp = tempdir();
    let img = tmp.join("photo2.png");
    std::fs::write(&img, b"fake").unwrap();
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
    let meta = meta_read::read_all(&img).unwrap();
    assert_eq!(meta.tags, vec!["UPPERCASE".to_string()]);
}

/// Regression: contentless FTS5 requires `contentless_delete=1` to allow the
/// DELETE that rebuild_fts_row_tx uses. Prior to migration 0002 this failed
/// with "cannot DELETE from contentless fts5 table", rolling back every
/// tag-update transaction.
#[test]
fn fts_delete_after_tag_update_works() {
    let dir = tempdir();
    let db_path = dir.join("test.db");
    let db = picorg_lib::db::Db::open(&db_path).expect("open db");

    // Insert a fake folder + image so we have somewhere to attach tags.
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO library_folders (path, added_at) VALUES ('C:\\test', 0)",
            [],
        )?;
        conn.execute(
            "INSERT INTO images (folder_id, path, filename, ext, size_bytes, mtime_ms)
             VALUES (1, 'C:\\test\\a.jpg', 'a.jpg', 'jpg', 1, 0)",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    // Apply the same metadata patch we'd apply from the UI.
    let patch = picorg_lib::types::MetadataPatch {
        title: Some(Some("Hello".into())),
        rating: Some(Some(4)),
        comment: None,
        tags: Some(vec!["vacation".into(), "sunset".into()]),
        tags_add: None,
        tags_remove: None,
    };
    picorg_lib::db::queries::apply_metadata_patch(&db, 1, &patch)
        .expect("apply_metadata_patch should succeed");

    // Re-read to prove the transaction actually committed.
    let details = picorg_lib::db::queries::get_image(&db, 1).expect("get_image");
    assert_eq!(details.summary.title.as_deref(), Some("Hello"));
    assert_eq!(details.summary.rating, Some(4));
    assert_eq!(
        details.tags,
        vec!["sunset".to_string(), "vacation".to_string()]
    );

    // Apply another patch to force the DELETE-then-INSERT FTS path.
    let patch2 = picorg_lib::types::MetadataPatch {
        title: Some(Some("Second".into())),
        rating: None,
        comment: Some(Some("A comment".into())),
        tags: Some(vec!["beach".into()]),
        tags_add: None,
        tags_remove: None,
    };
    picorg_lib::db::queries::apply_metadata_patch(&db, 1, &patch2)
        .expect("second patch should succeed");

    let details2 = picorg_lib::db::queries::get_image(&db, 1).expect("get_image 2");
    assert_eq!(details2.tags, vec!["beach".to_string()]);
    assert_eq!(details2.summary.title.as_deref(), Some("Second"));
    assert_eq!(details2.comment.as_deref(), Some("A comment"));
}

/// Regression: batch (multi-select) tag updates apply to every id. This is
/// the same DB path the UI's "Apply tag changes" button hits when multiple
/// images are selected — `tags_add` and `tags_remove` deltas per-image.
#[test]
fn batch_tag_add_persists_for_every_image() {
    let dir = tempdir();
    let db_path = dir.join("batch.db");
    let db = picorg_lib::db::Db::open(&db_path).expect("open db");

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO library_folders (path, added_at) VALUES ('C:\\test', 0)",
            [],
        )?;
        for i in 1..=3 {
            conn.execute(
                "INSERT INTO images (folder_id, path, filename, ext, size_bytes, mtime_ms)
                 VALUES (1, ?1, ?2, 'jpg', 1, 0)",
                rusqlite::params![
                    format!("C:\\test\\img{i}.jpg"),
                    format!("img{i}.jpg")
                ],
            )?;
        }
        Ok(())
    })
    .unwrap();

    // Seed image 2 with an existing tag so we prove tags_add is additive.
    picorg_lib::db::queries::apply_metadata_patch(
        &db,
        2,
        &picorg_lib::types::MetadataPatch {
            title: None,
            rating: None,
            comment: None,
            tags: Some(vec!["existing".into()]),
            tags_add: None,
            tags_remove: None,
        },
    )
    .unwrap();

    // Now simulate the UI-side batch: same patch, applied to all 3 ids.
    let batch_patch = picorg_lib::types::MetadataPatch {
        title: None,
        rating: None,
        comment: None,
        tags: None,
        tags_add: Some(vec!["holiday".into(), "beach".into()]),
        tags_remove: None,
    };
    for id in 1..=3 {
        picorg_lib::db::queries::apply_metadata_patch(&db, id, &batch_patch)
            .unwrap_or_else(|e| panic!("apply failed for id {id}: {e}"));
    }

    let d1 = picorg_lib::db::queries::get_image(&db, 1).unwrap();
    let d2 = picorg_lib::db::queries::get_image(&db, 2).unwrap();
    let d3 = picorg_lib::db::queries::get_image(&db, 3).unwrap();
    assert_eq!(d1.tags, vec!["beach".to_string(), "holiday".to_string()]);
    assert_eq!(
        d2.tags,
        vec![
            "beach".to_string(),
            "existing".to_string(),
            "holiday".to_string()
        ]
    );
    assert_eq!(d3.tags, vec!["beach".to_string(), "holiday".to_string()]);

    // And tags_remove trims correctly across the whole batch.
    let remove_patch = picorg_lib::types::MetadataPatch {
        title: None,
        rating: None,
        comment: None,
        tags: None,
        tags_add: None,
        tags_remove: Some(vec!["holiday".into()]),
    };
    for id in 1..=3 {
        picorg_lib::db::queries::apply_metadata_patch(&db, id, &remove_patch).unwrap();
    }
    let d1 = picorg_lib::db::queries::get_image(&db, 1).unwrap();
    let d2 = picorg_lib::db::queries::get_image(&db, 2).unwrap();
    assert_eq!(d1.tags, vec!["beach".to_string()]);
    assert_eq!(d2.tags, vec!["beach".to_string(), "existing".to_string()]);
}

/// Round-trip test for embedded XMP writing: given a real minimal JPEG,
/// `embed_xmp_in_source` must inject an APP1 XMP segment whose contents are
/// then recoverable via `extract_embedded_xmp`.
#[test]
fn embed_xmp_roundtrip_jpeg() {
    use picorg_lib::core::metadata::xmp;
    let tmp = tempdir();
    let img = tmp.join("real.jpg");
    // Smallest valid JPEG (grey 1x1). Bytes taken from a public-domain
    // "one byte of image data" JPEG. Contains: SOI, DQT, SOF0, DHT, SOS,
    // one MCU of grey data, EOI.
    let jpeg: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05,
        0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B,
        0x0B, 0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A,
        0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28,
        0x37, 0x29, 0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38,
        0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00, 0x01,
        0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03,
        0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D, 0x01, 0x02, 0x03, 0x00, 0x04,
        0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71,
        0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1,
        0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A,
        0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A,
        0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57,
        0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74,
        0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4,
        0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8,
        0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3,
        0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6,
        0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9,
        0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0xD2, 0xCF,
        0x20, 0xFF, 0xD9,
    ];
    std::fs::write(&img, jpeg).unwrap();

    // Sanity: no XMP embedded yet.
    let pre = xmp::extract_embedded_xmp(&img).unwrap();
    assert!(pre.is_none(), "expected no XMP before write");

    // Build a real XMP packet from a UserMetadata and embed it.
    let mut m = xmp::UserMetadata::default();
    m.title = Some("Batch Title".into());
    m.description = Some("Batch Comment".into());
    m.rating = Some(3);
    m.subjects = Some(vec!["batch".into(), "test".into()]);
    let packet = xmp::build_xmp_packet(&m);
    let wrote = xmp::embed_xmp_in_source(&img, packet.as_bytes()).unwrap();
    assert!(wrote, "embed_xmp_in_source should report success for JPEG");

    // Extract it back and confirm the packet we just wrote is what we get.
    let after = xmp::extract_embedded_xmp(&img).unwrap().expect("XMP present");
    let parsed = xmp::parse_user_metadata(&after).expect("parse ok");
    assert_eq!(parsed.title.as_deref(), Some("Batch Title"));
    assert_eq!(parsed.description.as_deref(), Some("Batch Comment"));
    assert_eq!(parsed.rating, Some(3));
    assert_eq!(
        parsed.subjects.as_deref().unwrap_or(&[]).iter().cloned().collect::<Vec<_>>(),
        vec!["batch".to_string(), "test".to_string()]
    );

    // The file must still be a valid JPEG (SOI…EOI intact) and end with 0xFFD9.
    let out = std::fs::read(&img).unwrap();
    assert_eq!(&out[0..2], &[0xFF, 0xD8], "SOI intact");
    assert_eq!(&out[out.len() - 2..], &[0xFF, 0xD9], "EOI intact");

    // Second write replaces (not stacks) the XMP segment: exactly one APP1
    // with our marker should be present.
    let mut m2 = m.clone();
    m2.subjects = Some(vec!["only-one".into()]);
    let packet2 = xmp::build_xmp_packet(&m2);
    xmp::embed_xmp_in_source(&img, packet2.as_bytes()).unwrap();
    let after2 = xmp::extract_embedded_xmp(&img).unwrap().unwrap();
    let parsed2 = xmp::parse_user_metadata(&after2).unwrap();
    assert_eq!(
        parsed2.subjects.as_deref().unwrap_or(&[]).iter().cloned().collect::<Vec<_>>(),
        vec!["only-one".to_string()],
        "second embed must fully replace the first"
    );
}

fn tempdir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join("picorg_test")
        .join(format!("t{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
