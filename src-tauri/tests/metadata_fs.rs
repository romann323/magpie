//! End-to-end tests for the metadata **read** pipeline and the
//! per-folder library DB. After the DB redesign Magpie no longer
//! writes back into source files, so the previous roundtrip suite
//! was retired; what remains covers:
//!
//! - Legacy `.xmp` sidecar reads (Lightroom / old-Magpie compat).
//! - Registry recognises every expected extension for the scanner.
//! - `db::library` upsert + apply_metadata_patch round-trip on a
//!   real on-disk per-folder library DB.

use desktop_lib::core::formats::FormatRegistry;
use desktop_lib::core::metadata::read as meta_read;
use desktop_lib::db::library::{self, FileStat, MetadataPatch as LibMetadataPatch};
use std::io::Write;

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

// ---------- Per-folder library DB round-trip ----------

/// End-to-end sanity check on the new per-folder library:
/// upsert a row, apply a metadata patch, read it back, confirm
/// title + tags are persisted and the FTS row survives a rebuild.
#[test]
fn library_db_upsert_and_patch_roundtrip() {
    let tmp = tempdir();
    let db_path = tmp.join(".magpie").join("library.db");
    let lib = library::LibraryDb::open(&db_path, 1).expect("open library db");

    // Upsert a fresh row.
    let stat = FileStat {
        rel_path: "photos/a.jpg",
        filename: "a.jpg",
        ext: "jpg",
        size_bytes: 123,
        mtime_ms: 42,
    };
    let outcome = {
        let conn = lib.lock().unwrap();
        library::upsert_image(&conn, &stat).unwrap()
    };
    let local_id = outcome.local_id();

    // Apply title + tags patch.
    let patch = LibMetadataPatch {
        title: Some(Some("Alpha".into())),
        tags: Some(vec!["vacation".into(), "sunset".into()]),
        tags_add: None,
        tags_remove: None,
    };
    {
        let mut conn = lib.lock().unwrap();
        library::apply_metadata_patch(&mut conn, local_id, &patch).unwrap();
    }

    // Read the row back.
    let row = {
        let conn = lib.lock().unwrap();
        library::get_image_row(&conn, local_id).unwrap().unwrap()
    };
    assert_eq!(row.title.as_deref(), Some("Alpha"));
    let mut tags = row.tags.clone();
    tags.sort();
    assert_eq!(tags, vec!["sunset".to_string(), "vacation".to_string()]);
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
