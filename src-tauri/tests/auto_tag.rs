//! Integration tests for the automatic-AI-tagging pipeline's DB
//! primitives. The full [`desktop_lib::core::auto_tag::tag_folder`]
//! flow needs an `AppHandle` + real `AppServices`, which we can't
//! construct outside a Tauri app, so these tests exercise the same
//! logic (candidate listing, patch application, fingerprint update)
//! end-to-end against a real on-disk `magpie.db`. The `MockClassifier`
//! itself is unit-tested inside `core::auto_tag::classifier`.

use desktop_lib::core::auto_tag::classifier::{ImageClassifier, MockClassifier};
use desktop_lib::db::queries::{self, FileStat, ImageMetaFromFile};
use desktop_lib::db::Db;
use std::path::Path;

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn tempdir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join("magpie_auto_tag_test")
        .join(format!("t{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn setup_folder_with_image(db: &Db, filename: &str, mtime_ms: i64) -> (i64, i64) {
    let folder = db
        .with_conn(|conn| queries::insert_folder(conn, Path::new(".")))
        .unwrap();
    let stat = FileStat {
        folder_id: folder.id,
        rel_path: filename.to_string(),
        filename,
        ext: "jpg",
        size_bytes: 100,
        mtime_ms,
    };
    let image_id = db
        .with_conn(|conn| queries::upsert_image(conn, &stat))
        .unwrap()
        .id();
    (folder.id, image_id)
}

/// Simulate one auto-tag pass: enumerate candidates, write the
/// classifier's suggestions as `'auto'`-source tags via
/// `add_auto_tags_for_image`, then record the fingerprint on the row.
/// Mirrors what `core::auto_tag::tag_one` does per image.
fn run_pass(db: &Db, folder_id: i64, classifier: &dyn ImageClassifier) -> Vec<(i64, Vec<String>)> {
    let candidates = db
        .with_conn(|conn| queries::list_auto_tag_candidates(conn, folder_id))
        .unwrap();
    let mut tagged: Vec<(i64, Vec<String>)> = Vec::new();
    for cand in candidates {
        if cand.ai_tag_hash.as_deref() == Some(cand.fingerprint.as_str()) {
            continue;
        }
        let mut suggestions = classifier.classify(cand.rel_path.as_bytes()).unwrap();
        suggestions.retain(|s| s.confidence >= classifier.min_confidence());
        suggestions.truncate(classifier.max_tags_per_image());
        let names: Vec<String> = suggestions.into_iter().map(|s| s.name).collect();
        if !names.is_empty() {
            db.with_conn_mut(|conn| queries::add_auto_tags_for_image(conn, cand.id, &names))
                .unwrap();
            tagged.push((cand.id, names));
        }
        db.with_conn(|conn| {
            queries::mark_image_ai_tagged(conn, cand.id, &cand.fingerprint, 999_000)
        })
        .unwrap();
    }
    tagged
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// A fresh folder with untagged images: every image should end up
/// with the classifier's suggestions attached as user tags, and the
/// row should carry an `ai_tag_hash` matching its fingerprint.
#[test]
fn first_pass_tags_every_image() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let (folder_id, id) = setup_folder_with_image(&db, "a.jpg", 42);

    let tagged = run_pass(&db, folder_id, &MockClassifier::new());
    assert_eq!(tagged.len(), 1, "one image should be tagged");
    assert!(!tagged[0].1.is_empty(), "at least one tag applied");

    let row = db
        .with_conn(|conn| queries::get_image_row(conn, id))
        .unwrap()
        .unwrap();
    assert!(
        !row.auto_tags.is_empty(),
        "auto_tags populated: {:?}",
        row.auto_tags
    );
    assert!(
        row.user_tags.is_empty(),
        "AI tags must not spill into user_tags: {:?}",
        row.user_tags
    );

    // Candidate listing now shows the fingerprint match — a second
    // pass will skip this row.
    let cands = db
        .with_conn(|conn| queries::list_auto_tag_candidates(conn, folder_id))
        .unwrap();
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].ai_tag_hash.as_deref(), Some("42"));
    assert_eq!(cands[0].fingerprint, "42");
}

/// The second AI pass on an unchanged folder must be a no-op —
/// nothing new gets tagged.
#[test]
fn second_pass_skips_unchanged_images() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let (folder_id, _id) = setup_folder_with_image(&db, "a.jpg", 42);

    let first = run_pass(&db, folder_id, &MockClassifier::new());
    assert_eq!(first.len(), 1);

    let second = run_pass(&db, folder_id, &MockClassifier::new());
    assert!(
        second.is_empty(),
        "second pass must skip already-tagged rows: {second:?}"
    );
}

/// After a file's mtime changes (as would happen when the user
/// re-edits or re-imports it), the fingerprint no longer matches so
/// the AI pass reclassifies the image.
#[test]
fn changed_fingerprint_reclassifies() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let (folder_id, id) = setup_folder_with_image(&db, "a.jpg", 42);
    let _ = run_pass(&db, folder_id, &MockClassifier::new());

    // Simulate a rescan that bumps mtime → this should invalidate
    // the previous fingerprint.
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE images SET mtime_ms = ?1 WHERE id = ?2",
            rusqlite::params![100, id],
        )?;
        Ok(())
    })
    .unwrap();

    let again = run_pass(&db, folder_id, &MockClassifier::new());
    assert_eq!(again.len(), 1, "changed fingerprint must re-trigger AI");

    // Fingerprint on disk now reflects the new mtime.
    let cands = db
        .with_conn(|conn| queries::list_auto_tag_candidates(conn, folder_id))
        .unwrap();
    assert_eq!(cands[0].ai_tag_hash.as_deref(), Some("100"));
}

/// AI-supplied tags land as `'auto'` tags — they show up in
/// `auto_tags` (never `user_tags`), coexist with XMP-derived auto
/// tags without duplication, and don't clobber user edits typed
/// against the same image.
#[test]
fn ai_tags_land_as_auto_and_coexist_with_scanner() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let (folder_id, id) = setup_folder_with_image(&db, "a.jpg", 42);

    // 1. Scanner writes one auto tag from the file itself.
    let scan = ImageMetaFromFile {
        tags: vec!["sunset".into()],
        ..Default::default()
    };
    db.with_conn_mut(|conn| queries::set_image_meta(conn, id, &scan))
        .unwrap();

    // 2. User types a tag through the UI (lands as `'user'`).
    db.with_conn_mut(|conn| {
        queries::apply_metadata_patch(
            conn,
            id,
            &desktop_lib::db::queries::MetadataPatch {
                title: None,
                tags: Some(vec!["keeper".into()]),
                tags_add: None,
                tags_remove: None,
            },
        )
    })
    .unwrap();

    // 3. AI pass — must add auto rows only, never touch the user
    //    side.
    let _ = run_pass(&db, folder_id, &MockClassifier::new());

    let row = db
        .with_conn(|conn| queries::get_image_row(conn, id))
        .unwrap()
        .unwrap();
    assert!(row.auto_tags.contains(&"sunset".to_string()));
    assert!(row.auto_tags.len() >= 2, "AI added at least one auto tag: {:?}", row.auto_tags);
    assert_eq!(
        row.user_tags,
        vec!["keeper".to_string()],
        "AI pass must leave user tags alone"
    );

    // Aggregated view: union of everything, deduped.
    let mut all = db
        .with_conn(|conn| queries::tags_for_image(conn, id))
        .unwrap();
    all.sort();
    let mut expected: Vec<String> = row
        .auto_tags
        .iter()
        .chain(row.user_tags.iter())
        .cloned()
        .collect();
    expected.sort();
    expected.dedup();
    assert_eq!(all, expected);
}

/// The migration path leaves existing images with NULL `ai_tag_hash`
/// (they've never been AI-tagged); `list_auto_tag_candidates` must
/// surface them so the first pass picks them up.
#[test]
fn candidates_include_never_tagged_rows() {
    let tmp = tempdir();
    let db = Db::open(&tmp.join("magpie.db")).unwrap();
    let (folder_id, _) = setup_folder_with_image(&db, "a.jpg", 42);
    let cands = db
        .with_conn(|conn| queries::list_auto_tag_candidates(conn, folder_id))
        .unwrap();
    assert_eq!(cands.len(), 1);
    assert!(cands[0].ai_tag_hash.is_none());
}
