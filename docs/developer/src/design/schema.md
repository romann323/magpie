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
| ai_tagged_at    | INTEGER | Nullable; Unix ms of the last successful AI-tag pass.    |
| ai_tag_hash     | TEXT    | Nullable; fingerprint (`content_hash` or `mtime_ms`) at the time of that pass; the AI pipeline skips a row on re-run when this still matches. |

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

Many-to-many join. Every row records **who attached the tag** via the
`source` column:

- `'auto'` — imported from the file's own metadata (XMP subjects,
  Windows Shell keywords, sidecar XMP) at scan time.
- `'user'` — added by the user inside Magpie via the DetailsPanel.

The same `(image, tag)` pair may exist twice — once from each source —
and the sidebar / FTS index treat them as one tag when aggregating.

| Column     | Type    | Notes                                            |
| ---------- | ------- | ------------------------------------------------ |
| image_id   | INTEGER | FK → `images(id)` ON DELETE CASCADE.             |
| tag_id     | INTEGER | FK → `tags(id)` ON DELETE CASCADE.               |
| source     | TEXT    | `CHECK (source IN ('auto','user'))`.             |
| **PK**     |         | `(image_id, tag_id, source)`.                    |

Indexes:

- `idx_image_tags_tag(tag_id)` — vocabulary → images.
- `idx_image_tags_source(image_id, source)` — one image's tags of a
  given kind, used by `user_tags_for_image` /
  `auto_tags_for_image` in `db::queries`.

**Who writes what:**

- The **scanner path** (`db::queries::set_image_meta`, called on scan
  and on any mtime-triggered re-read) inserts `'auto'` rows only for
  names the image doesn't already carry (in either source). It never
  deletes anything, so a user tag survives a rescan even when the
  file's own metadata no longer mentions it, and an auto tag stays put
  even if the source file drops it.
- **User edits** (`db::queries::apply_metadata_patch` from the
  DetailsPanel) only ever touch `'user'` rows.
  `MetadataPatch.tagsRemove` deletes the matching `'user'` row and
  leaves any `'auto'` row alone.

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

`schema_meta.schema_version` is bumped when the schema changes.
`db::schema::apply` runs upgrades one hop at a time; every hop lives
in its own `migrate_vN_to_vN+1` helper.

Versions:

- **v1** — initial single-DB layout.
- **v2** — split `image_tags` by provenance. `source TEXT NOT NULL
  CHECK (source IN ('auto','user'))` is added and the PK becomes
  `(image_id, tag_id, source)`. Existing rows have no provenance, so
  the migration marks them all as `'user'`; the next scan adds any
  `'auto'` rows the format handlers pick up alongside without
  disturbing user edits.
- **v3** — add automatic-AI-tagging bookkeeping to `images`:
  `ai_tagged_at INTEGER` and `ai_tag_hash TEXT`. Both nullable so
  existing rows migrate in place. The auto-tag pipeline (see
  [Scanner](./scanner.md) and `core::auto_tag`) writes both columns
  once per successful classifier pass and compares `ai_tag_hash`
  against a per-image fingerprint on the next run to decide whether
  the image can be skipped.

Two legacy on-disk layouts are recognised at startup and imported
one-shot into `magpie.db`:

1. **Pre-redesign** — `library.db` in the app-data dir.
2. **Per-folder redesign** — `registry.db` in the app-data dir plus
   `<folder>\.magpie\library.db` per registered folder.

See [Database design](./db-redesign.md) for the migration algorithm.
Legacy files are never deleted in place — they get a
`.migrated-<yyyymmddThhmmss>` suffix so the user can inspect or
restore them.
