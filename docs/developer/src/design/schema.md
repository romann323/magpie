# Database schema

Everything Magpie persists lives in one SQLite file: `magpie.db` in
the app-data directory. See [Database design](./db-redesign.md) for
the motivation and the two earlier layouts we've migrated away from.

**Location:** `%APPDATA%\com.magpie.app\magpie.db` (Windows).
**Owner:** `src-tauri/src/db/schema.rs` reads the DDL from
`schema.sql` and applies it verbatim on a fresh file.
**Pragmas:** WAL journal, `synchronous=NORMAL`, foreign keys on,
`busy_timeout=5000`.

## Tables

### `library_folders`

Registered root folders. Grows with the number of folders the user
adds, not with the number of files.

| Column        | Type    | Notes                                              |
| ------------- | ------- | -------------------------------------------------- |
| **id**        | INTEGER | PK, autoincrement.                                 |
| path          | TEXT    | Absolute canonical path, UNIQUE COLLATE NOCASE.    |
| added_at      | INTEGER | Unix ms.                                           |
| last_scan_at  | INTEGER | Unix ms; NULL before first scan finishes.          |
| is_available  | INTEGER | 0/1; 0 when the folder root can't be reached.      |

### `images`

One row per file (of any format the registry recognises) scanned into
any library folder. Table name is historical; it now holds videos,
PDFs, and documents too.

| Column          | Type    | Notes                                                    |
| --------------- | ------- | -------------------------------------------------------- |
| **id**          | INTEGER | PK, autoincrement. Globally unique.                      |
| folder_id       | INTEGER | FK → `library_folders(id)` ON DELETE CASCADE.            |
| rel_path        | TEXT    | Folder-relative, forward slashes.                        |
| filename        | TEXT    | Basename (`IMG_1234.jpg`).                               |
| ext             | TEXT    | Lowercase, no leading dot.                               |
| size_bytes      | INTEGER | From `fs::Metadata::len()`.                              |
| mtime_ms        | INTEGER | Modification time (Unix ms).                             |
| width           | INTEGER | Nullable; from handler's technical read.                 |
| height          | INTEGER | Nullable.                                                |
| content_hash    | TEXT    | Nullable; XXH3 128-bit hex digest.                       |
| taken_at        | INTEGER | Nullable; Unix ms from EXIF `DateTimeOriginal`.          |
| camera_make     | TEXT    | Nullable.                                                |
| camera_model    | TEXT    | Nullable.                                                |
| title           | TEXT    | Nullable; user-editable.                                 |
| imported_at     | INTEGER | Unix ms when the row was first inserted.                 |
| missing         | INTEGER | 0/1; 1 after a scan that failed to see the file.         |

**UNIQUE:** `(folder_id, rel_path)` — the scanner upserts on this key.
**Indexes:** `idx_images_folder`, `idx_images_taken_at`,
`idx_images_filename`, `idx_images_ext`, `idx_images_hash`.

### `tags`

| Column | Type    | Notes                              |
| ------ | ------- | ---------------------------------- |
| **id** | INTEGER | PK, autoincrement.                 |
| name   | TEXT    | UNIQUE COLLATE NOCASE.             |

Global vocabulary. `"Beach"` and `"beach"` end up in the same row.

### `image_tags`

Many-to-many join.

| Column     | Type    | Notes                                     |
| ---------- | ------- | ----------------------------------------- |
| image_id   | INTEGER | FK → `images(id)` ON DELETE CASCADE.      |
| tag_id     | INTEGER | FK → `tags(id)` ON DELETE CASCADE.        |
| **PK**     |         | `(image_id, tag_id)`.                     |

Index: `idx_image_tags_tag(tag_id)`.

### `images_fts` (virtual, FTS5)

```sql
CREATE VIRTUAL TABLE images_fts USING fts5(
    title, filename, tags,
    content='',
    contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2'
);
```

- Contentless (`content=''`) — `rowid` mirrors `images.id`; we join
  from FTS matches back to the base table.
- `contentless_delete=1` — required for our rebuild-per-row strategy
  (`DELETE + INSERT` inside every metadata patch).
- Diacritics folded (`naive` = `naïve`).

### `smart_collections`

| Column     | Type    | Notes                                     |
| ---------- | ------- | ----------------------------------------- |
| **id**     | INTEGER | PK, autoincrement.                        |
| name       | TEXT    |                                           |
| filter     | TEXT    | JSON-serialised `ImageFilter`.            |
| sort_order | INTEGER |                                           |

### `app_settings`

Key-value store for app-wide preferences the UI persists across
launches.

| Column  | Type | Notes                                     |
| ------- | ---- | ----------------------------------------- |
| **key** | TEXT | PK.                                       |
| value   | TEXT | Free-form (JSON, plain string, …).        |

### `schema_meta` (singleton, row `id = 1`)

| Column          | Type    | Notes                                              |
| --------------- | ------- | -------------------------------------------------- |
| **id**          | INTEGER | Constant `1`.                                      |
| magpie_version  | TEXT    | `CARGO_PKG_VERSION` when the DB was created.       |
| schema_version  | INTEGER | Bump on breaking schema changes.                   |
| created_at      | INTEGER | Unix ms.                                           |

Used by `db::schema::apply` to decide whether the DB is fresh (no
`schema_meta` table → run DDL) or upgradable (row present → check
version).

## Relationships

```
library_folders (1) ─────┐
                         │ folder_id
                         ▼
                       images ────── (rowid = id) ──── images_fts
                         ▲
                         │ image_id
                         ▼
                     image_tags
                         ▲
                         │ tag_id
                         ▼
                        tags
```

## Global IDs

There is no packing scheme. `images.id` is a plain SQLite
autoincrement primary key and is unique across the whole app. The IPC
layer forwards it verbatim; the frontend treats it as opaque.

## Migrations

`schema_meta.schema_version` is bumped when the schema changes. Only
version 1 exists today. Future migrations plug into
`db::schema::apply` between the "existing DB" branch and the "version
matches" check.

Two legacy on-disk layouts are recognised at startup and imported
one-shot into `magpie.db`:

1. **Pre-redesign** — `library.db` in the app-data dir.
2. **Per-folder redesign** — `registry.db` in the app-data dir plus
   `<folder>\.magpie\library.db` per registered folder.

See [Database design](./db-redesign.md) for the migration algorithm.
Legacy files are never deleted in place — they get a
`.migrated-<yyyymmddThhmmss>` suffix so the user can inspect or
restore them.
