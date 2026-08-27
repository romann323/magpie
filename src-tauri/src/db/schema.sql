-- Magpie central DB schema (v1).
-- Applied verbatim on a fresh magpie.db by db::schema::apply.

PRAGMA foreign_keys = ON;

-- Registered library roots.
CREATE TABLE IF NOT EXISTS library_folders (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    path         TEXT    NOT NULL UNIQUE COLLATE NOCASE, -- absolute canonical
    added_at     INTEGER NOT NULL,
    last_scan_at INTEGER,
    is_available INTEGER NOT NULL DEFAULT 1
);

-- One row per file the scanner has seen. `id` is a plain autoincrement
-- integer — globally unique across all folders.
CREATE TABLE IF NOT EXISTS images (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id    INTEGER NOT NULL
                 REFERENCES library_folders(id) ON DELETE CASCADE,
    rel_path     TEXT    NOT NULL,   -- folder-relative, forward slashes
    filename     TEXT    NOT NULL,
    ext          TEXT    NOT NULL,   -- lower-case, no leading dot
    size_bytes   INTEGER NOT NULL,
    mtime_ms     INTEGER NOT NULL,
    width        INTEGER,
    height       INTEGER,
    content_hash TEXT,
    taken_at     INTEGER,
    camera_make  TEXT,
    camera_model TEXT,
    title        TEXT,
    imported_at  INTEGER NOT NULL,
    missing      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (folder_id, rel_path)
);

CREATE INDEX IF NOT EXISTS idx_images_folder   ON images(folder_id);
CREATE INDEX IF NOT EXISTS idx_images_taken_at ON images(taken_at);
CREATE INDEX IF NOT EXISTS idx_images_filename ON images(filename);
CREATE INDEX IF NOT EXISTS idx_images_ext      ON images(ext);
CREATE INDEX IF NOT EXISTS idx_images_hash     ON images(content_hash);

-- Global tag vocabulary.
CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

-- Many-to-many join.
CREATE TABLE IF NOT EXISTS image_tags (
    image_id INTEGER NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id)   ON DELETE CASCADE,
    PRIMARY KEY (image_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_image_tags_tag ON image_tags(tag_id);

-- FTS5 index over title, filename, and comma-joined tag names.
CREATE VIRTUAL TABLE IF NOT EXISTS images_fts USING fts5(
    title, filename, tags,
    content='',
    contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2'
);

-- Persisted saved searches.
CREATE TABLE IF NOT EXISTS smart_collections (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    filter     TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);

-- Free-form key-value store for UI-level preferences.
CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Singleton row describing the DB itself.
CREATE TABLE IF NOT EXISTS schema_meta (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    magpie_version TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    created_at     INTEGER NOT NULL
);
