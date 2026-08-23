# DB redesign: per-folder SQLite + central registry

## Motivation

Prior to the `DB-redesign` branch, Magpie treated the file itself as the
source of truth for user metadata. Titles and tags were embedded into
JPEG/PNG/WebP/GIF via XMP and into every other format via the Windows
Shell property system (`SHGetPropertyStoreFromParsingName`). The central
SQLite database in `%APPDATA%\com.magpie.app\library.db` was just an
index/cache used to make search fast.

That design had three sharp edges:

1. **Formats without a writable property handler couldn't be tagged at
   all.** BMP, DIB, SVG, EXR, HDR, PSD, JPEG 2000, several MPEG-TS
   variants — all locked out on Windows.
2. **Read/write parity across formats was hard.** JPEG got a
   hand-written XMP writer, PNG got `iTXt`, WebP got a RIFF chunk, GIF
   got the Application Extension trailer, and everything else went
   through Windows COM with its own bag of failure modes.
3. **Multi-folder libraries had to share one central DB.** Moving a
   folder to another PC, backing it up, or handing it to a colleague
   meant losing all the tagging work unless the user manually copied the
   central DB too.

The redesign flips the model: **the database is the sole source of
truth for user metadata**, and each library folder gets its own DB
sitting inside it as `<folder>/.magpie/library.db`.

## Two-tier layout

```
Central registry DB (per-machine)     Per-folder library DBs (per-folder, portable)
─────────────────────────────────     ────────────────────────────────────────────
%APPDATA%\com.magpie.app\             <folder1>\.magpie\library.db
    registry.db                       <folder2>\.magpie\library.db
    ├─ library_folders                <folder3>\.magpie\library.db
    ├─ smart_collections                  ├─ folder_meta (singleton)
    └─ app_settings                       ├─ images (rel_path, ...)
                                          ├─ tags
                                          ├─ image_tags
                                          └─ images_fts (FTS5)
```

### Registry DB

Lives at `%APPDATA%\com.magpie.app\registry.db`. Small — grows with the
number of registered folders, not the number of files. Schema:

```sql
CREATE TABLE library_folders (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    path          TEXT NOT NULL UNIQUE,            -- absolute folder path
    added_at      INTEGER NOT NULL,
    last_scan_at  INTEGER,
    is_available  INTEGER NOT NULL DEFAULT 1       -- 0 = folder missing (removable drive unmounted)
);

CREATE TABLE smart_collections (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    filter     TEXT NOT NULL,                      -- JSON-encoded ImageFilter
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### Library DB

Lives at `<folder>/.magpie/library.db`. Fully self-contained: every
path is stored relative to `<folder>`, and tag names are stored inline
(no reference to a global tag ID table). Copy the folder to another PC
or archive it as a `.zip` and everything works.

```sql
CREATE TABLE folder_meta (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    magpie_version  TEXT NOT NULL,
    schema_version  INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    last_scan_at    INTEGER
);

CREATE TABLE images (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    rel_path      TEXT NOT NULL UNIQUE,            -- relative to folder root, forward slashes
    filename      TEXT NOT NULL,
    ext           TEXT NOT NULL,
    size_bytes    INTEGER NOT NULL,
    mtime_ms      INTEGER NOT NULL,
    width         INTEGER,
    height        INTEGER,
    content_hash  TEXT,
    taken_at      INTEGER,
    camera_make   TEXT,
    camera_model  TEXT,
    title         TEXT,
    imported_at   INTEGER NOT NULL,
    missing       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_images_taken_at ON images(taken_at);
CREATE INDEX idx_images_filename ON images(filename);
CREATE INDEX idx_images_ext      ON images(ext);

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
    title, filename, tags,
    content='',
    contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2'
);
```

## Global ID scheme

Every image row must have a *globally unique* ID that the frontend can
pass through `getImage(id)`, invalidate queries by, etc. — but the
per-folder DB only knows its own local IDs starting at 1.

Packed encoding:

```rust
pub const FOLDER_ID_MULT: i64 = 1_000_000_000;

pub fn pack(folder_id: i64, image_id: i64) -> i64 {
    folder_id * FOLDER_ID_MULT + image_id
}
pub fn unpack(global_id: i64) -> (i64, i64) {
    (global_id / FOLDER_ID_MULT, global_id % FOLDER_ID_MULT)
}
```

Room for **9 M folders × 1 B images each**, all inside JavaScript's
`Number.MAX_SAFE_INTEGER` (2⁵³ ≈ 9 × 10¹⁵) so no BigInt gymnastics on
the frontend.

## Cross-folder search via `ATTACH DATABASE`

At startup the registry connection attaches every registered library
DB:

```sql
ATTACH DATABASE 'C:\photos\2023\.magpie\library.db' AS f1;
ATTACH DATABASE 'C:\photos\2024\.magpie\library.db' AS f2;
```

Global queries become a `UNION ALL` across attached schemas, with the
folder ID synthesised into each row's ID column at select time:

```sql
SELECT (1 * 1000000000 + id) AS gid, 1 AS folder_id, ... FROM f1.images WHERE missing = 0
UNION ALL
SELECT (2 * 1000000000 + id) AS gid, 2 AS folder_id, ... FROM f2.images WHERE missing = 0
```

FTS across libraries works identically — each library has its own
`images_fts` and we `UNION` the matches.

SQLite allows up to 125 attached DBs per connection with
`SQLITE_LIMIT_ATTACHED`, and we lift the limit from the default 10 in
`LibraryPool::open`.

Attach/detach is amortized: it happens exactly once when a folder is
added or removed, never per query. The registry connection lives for
the app lifetime; the `LibraryPool` mediates access to the individual
library DBs when we need a *single-folder* write (scanning, tag edits).

## `LibraryPool`

New type in `src-tauri/src/db/pool.rs`:

```rust
pub struct LibraryPool {
    /// Registry-owning connection; every library DB is ATTACHed here for
    /// cross-folder reads. Guarded by a Mutex — search reads and folder
    /// add/remove serialize through it, which is fine because those are
    /// low-frequency.
    reg: Arc<Mutex<Connection>>,
    /// Per-folder writer connections. Populated lazily.
    libraries: RwLock<HashMap<i64, Arc<LibraryDb>>>,
    /// folder_id → filesystem path of that library DB, snapshot from the
    /// registry, kept in sync via add/remove_folder.
    library_paths: RwLock<HashMap<i64, PathBuf>>,
}
```

Read path (cross-folder queries):
- `LibraryPool::with_registry_conn(|conn| ...)` locks the registry
  mutex and hands the caller the attached connection. Callers build
  `UNION ALL` queries against `f1.images`, `f2.images`, ...

Write path (single-folder edits, scanning):
- `LibraryPool::library(folder_id) -> Arc<LibraryDb>` returns a
  writer connection for that folder's DB, opening it if needed. All
  writes to a folder serialize through its own connection Mutex —
  different folders can be written in parallel.

## Reading pre-existing embedded tags on first scan

Format handlers keep `read_user` and `read_technical`. The scanner
still calls them for every file, so a Lightroom-tagged JPEG or a
Windows-Explorer-tagged X3F imports its tags into the per-folder DB
on the first scan. After that the DB is authoritative and the file
bytes are never touched.

`FormatHandler::write_user` and `FormatHandler::can_write_tags` are
removed from the trait. `win_shell::write_user_meta`,
`win_shell::can_write_tags`, `common::atomic_write_bytes`,
`xmp_packet::build_xmp_packet`, and `xmp_packet::merge_user_edits`
are deleted along with the entire `core::metadata::write` module.

## Migration from the legacy central DB

On `AppServices::new`, if `%APPDATA%\com.magpie.app\registry.db` does
**not** exist but `library.db` (the old central DB) *does*, run the
legacy migration:

1. Open the old DB read-only.
2. For every row in `library_folders`:
   a. `mkdir <folder>/.magpie`
   b. Create a fresh `library.db` there with the new schema.
   c. Copy `images` rows for that folder, translating absolute
      `images.path` → folder-relative `images.rel_path`.
   d. Copy `tags` for those images (via `image_tags`), preserving
      names.
   e. Rebuild `images_fts`.
3. Create the new `registry.db`, insert one `library_folders` row per
   migrated folder.
4. Rename the old file to `library.db.migrated-<yyyymmddThhmmss>` so
   nothing is destroyed.

Idempotent: skipped if `registry.db` already exists. Recoverable: if
migration fails partway through, the legacy DB is still intact and
the process retries on the next launch (registry.db is only written
after every folder migrated successfully).

## Sync-location warning

Because the library DB lives *inside* the folder, putting the folder
in a sync location (OneDrive, Dropbox, Google Drive, iCloud) exposes
the DB to concurrent-writer risk when two PCs edit tags
simultaneously.

`commands::library::check_folder_sync_risk(path)` returns
`Option<String>` — a human-readable warning if the path matches known
sync-provider patterns or lives on a network share:

- Path contains `\OneDrive` (with or without `- <tenant>` suffix).
- Path contains `\Dropbox`.
- Path contains `\Google Drive` or `\GoogleDrive`.
- Path contains `\iCloudDrive` / `\iCloud Drive`.
- Path starts with `\\` (UNC / network share).

The frontend calls this before `add_library_folder` and shows a
`confirm` dialog with the warning when non-null. The user can still
proceed; Magpie just makes sure they know.

## What the frontend sees change

- `ImageDetails` loses `writeMode`, `canWriteTags`, `metaWrittenAt`
  fields.
- No amber "library-only" hint under the tags editor — every file is
  now DB-taggable.
- `ImageSummary.id` is now a *packed global ID*
  (`folder_id * 1_000_000_000 + local_id`).
- `ImageSummary.path` is still the absolute path (registry + library
  join it back together for the UI, thumbnails, and inline preview).
- New IPC command `check_folder_sync_risk(path) -> Option<String>`.

## What the file bytes see change

Magpie **never writes back into the source file** after this redesign.

- No XMP packets are inserted or updated.
- No Windows Shell IPropertyStore SetValue calls.
- No `.magpie.tmp` scratch files, no atomic rename dance for the
  source file.
- The file's `mtime` is not touched by Magpie unless the user
  explicitly deletes it via *Move to Recycle Bin*.

Existing embedded tags are read exactly once, at first scan, and
imported into the per-folder DB. From then on the DB is the truth.
