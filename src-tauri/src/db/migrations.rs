use crate::error::AppResult;
use rusqlite::{params, Connection};

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_init",
        r#"
        CREATE TABLE library_folders (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            path          TEXT NOT NULL UNIQUE,
            added_at      INTEGER NOT NULL,
            last_scan_at  INTEGER
        );

        CREATE TABLE images (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            folder_id       INTEGER NOT NULL REFERENCES library_folders(id) ON DELETE CASCADE,
            path            TEXT NOT NULL UNIQUE,
            filename        TEXT NOT NULL,
            ext             TEXT NOT NULL,
            size_bytes      INTEGER NOT NULL,
            mtime_ms        INTEGER NOT NULL,
            width           INTEGER,
            height          INTEGER,
            content_hash    TEXT,
            taken_at        INTEGER,
            camera_make     TEXT,
            camera_model    TEXT,
            title           TEXT,
            rating          INTEGER CHECK (rating IS NULL OR (rating BETWEEN 0 AND 5)),
            comment         TEXT,
            meta_written_at INTEGER,
            meta_read_at    INTEGER,
            missing         INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX idx_images_folder   ON images(folder_id);
        CREATE INDEX idx_images_taken_at ON images(taken_at);
        CREATE INDEX idx_images_rating   ON images(rating);
        CREATE INDEX idx_images_filename ON images(filename);

        CREATE TABLE tags (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE
        );

        CREATE TABLE image_tags (
            image_id INTEGER NOT NULL REFERENCES images(id) ON DELETE CASCADE,
            tag_id   INTEGER NOT NULL REFERENCES tags(id)   ON DELETE CASCADE,
            PRIMARY KEY (image_id, tag_id)
        );

        CREATE INDEX idx_image_tags_tag ON image_tags(tag_id);

        CREATE VIRTUAL TABLE images_fts USING fts5(
            title, comment, filename, tags,
            content='',
            tokenize='unicode61 remove_diacritics 2'
        );

        CREATE TABLE smart_collections (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL,
            filter     TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0
        );
        "#,
    ),
    (
        // Fix: the original images_fts table was contentless without the
        // `contentless_delete=1` option, so `DELETE FROM images_fts` (used by
        // rebuild_fts_row_tx) always failed with
        //   "cannot DELETE from contentless fts5 table: images_fts".
        //
        // Every transaction that touched tags/title/comment therefore rolled
        // back, silently discarding user edits and scan-detected metadata.
        //
        // This migration recreates the FTS table with contentless_delete=1
        // (SQLite 3.43+), repopulates it from images/image_tags, and clears
        // meta_read_at so the next call to get_image / rescan will re-read
        // metadata from disk into the DB properly.
        "0002_fts_contentless_delete",
        r#"
        DROP TABLE IF EXISTS images_fts;

        CREATE VIRTUAL TABLE images_fts USING fts5(
            title, comment, filename, tags,
            content='',
            contentless_delete=1,
            tokenize='unicode61 remove_diacritics 2'
        );

        INSERT INTO images_fts(rowid, title, comment, filename, tags)
        SELECT
            i.id,
            COALESCE(i.title, ''),
            COALESCE(i.comment, ''),
            i.filename,
            COALESCE(
                (SELECT GROUP_CONCAT(t.name, ' ')
                 FROM tags t JOIN image_tags it ON it.tag_id = t.id
                 WHERE it.image_id = i.id),
                ''
            )
        FROM images i;

        -- Force get_image / rescan to pull metadata from FS again for every
        -- image, because prior transactions have been silently rolled back.
        UPDATE images SET meta_read_at = NULL;
        "#,
    ),
    (
        // Drop the `rating` and `comment` columns from `images`. Magpie's UI
        // no longer exposes these — the only user-editable metadata is Title
        // and Tags. Existing values on disk (xmp:Rating, dc:description) are
        // still readable by other tools; Magpie simply ignores them on write.
        //
        // Also recreate `images_fts` without the `comment` column, and clear
        // meta_read_at so the next scan re-indexes cleanly.
        "0003_drop_rating_comment",
        r#"
        BEGIN;

        DROP INDEX IF EXISTS idx_images_rating;

        CREATE TABLE images_new (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            folder_id       INTEGER NOT NULL REFERENCES library_folders(id) ON DELETE CASCADE,
            path            TEXT NOT NULL UNIQUE,
            filename        TEXT NOT NULL,
            ext             TEXT NOT NULL,
            size_bytes      INTEGER NOT NULL,
            mtime_ms        INTEGER NOT NULL,
            width           INTEGER,
            height          INTEGER,
            content_hash    TEXT,
            taken_at        INTEGER,
            camera_make     TEXT,
            camera_model    TEXT,
            title           TEXT,
            meta_written_at INTEGER,
            meta_read_at    INTEGER,
            missing         INTEGER NOT NULL DEFAULT 0
        );

        INSERT INTO images_new
            (id, folder_id, path, filename, ext, size_bytes, mtime_ms,
             width, height, content_hash, taken_at, camera_make, camera_model,
             title, meta_written_at, meta_read_at, missing)
        SELECT
             id, folder_id, path, filename, ext, size_bytes, mtime_ms,
             width, height, content_hash, taken_at, camera_make, camera_model,
             title, meta_written_at, meta_read_at, missing
        FROM images;

        DROP TABLE images;
        ALTER TABLE images_new RENAME TO images;

        CREATE INDEX idx_images_folder   ON images(folder_id);
        CREATE INDEX idx_images_taken_at ON images(taken_at);
        CREATE INDEX idx_images_filename ON images(filename);

        DROP TABLE IF EXISTS images_fts;
        CREATE VIRTUAL TABLE images_fts USING fts5(
            title, filename, tags,
            content='',
            contentless_delete=1,
            tokenize='unicode61 remove_diacritics 2'
        );

        INSERT INTO images_fts(rowid, title, filename, tags)
        SELECT
            i.id,
            COALESCE(i.title, ''),
            i.filename,
            COALESCE(
                (SELECT GROUP_CONCAT(t.name, ' ')
                 FROM tags t JOIN image_tags it ON it.tag_id = t.id
                 WHERE it.image_id = i.id),
                ''
            )
        FROM images i;

        -- Any smart collection that filtered by rating/comment is now stale.
        -- Rather than migrate JSON, drop the filter-by-rating clauses by
        -- wiping the whole filter (users can recreate). This is only
        -- reachable if a user actually created smart collections in v1.
        UPDATE smart_collections SET filter = '{}' WHERE filter LIKE '%rating%' OR filter LIKE '%comment%';

        UPDATE images SET meta_read_at = NULL;

        COMMIT;
        "#,
    ),
];

pub fn run(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    for (name, sql) in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT 1 FROM _migrations WHERE name = ?1",
                params![name],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !applied {
            tracing::info!(migration = %name, "applying migration");
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO _migrations (name, applied_at) VALUES (?1, ?2)",
                params![name, chrono::Utc::now().timestamp_millis()],
            )?;
        }
    }

    Ok(())
}
