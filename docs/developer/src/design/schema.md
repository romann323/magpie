# Database schema

Magpie's storage is **two-tier**: one small central registry DB per
machine, one library DB per registered folder that lives inside the
folder itself. See [Database redesign](./db-redesign.md) for the
motivation and cross-folder query strategy.

- **Registry DB** — `%APPDATA%\com.magpie.app\registry.db`.
  Owned schema in `src-tauri/src/db/registry.rs`.
- **Library DB** — `<folder>/.magpie/library.db`, one per registered
  folder. Owned schema in `src-tauri/src/db/library.rs`.

## Registry DB (`registry.db`)

Small. Grows with the number of registered folders, not the number
of files.

### `library_folders`

| Column        | Type    | Notes                                                    |
| ------------- | ------- | -------------------------------------------------------- |
| **id**        | INTEGER | PK, autoincrement.                                       |
| path          | TEXT    | Absolute, canonical, UNIQUE.                             |
| added_at      | INTEGER | Unix ms.                                                 |
| last_scan_at  | INTEGER | Unix ms; NULL before first scan finishes.                |
| is_available  | INTEGER | 0/1. 0 = `.magpie/library.db` couldn't be attached.      |

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

| Column | Type    | Notes                                     |
| ------ | ------- | ----------------------------------------- |
| **key**| TEXT    | PK.                                       |
| value  | TEXT    | Free-form (JSON, plain string, …).        |

## Library DB (`<folder>/.magpie/library.db`)

Fully self-contained: paths are folder-relative, tag names are
stored inline, the FTS index sits next to the tags table. Copying
the folder to a different disk/PC preserves everything.

### `folder_meta` (singleton, row `id = 1`)

| Column          | Type    | Notes                                             |
| --------------- | ------- | ------------------------------------------------- |
| **id**          | INTEGER | Constant `1`.                                     |
| magpie_version  | TEXT    | `CARGO_PKG_VERSION` when the DB was created.      |
| schema_version  | INTEGER | Bump on breaking schema changes.                  |
| created_at      | INTEGER | Unix ms.                                          |
| last_scan_at    | INTEGER | Nullable.                                         |

### `images`

One row per file (of any format the registry recognises) scanned in
this folder. The table is still called `images` for historical
reasons; a future migration may rename it to `files`.

| Column          | Type    | Notes                                                       |
| --------------- | ------- | ----------------------------------------------------------- |
| **id**          | INTEGER | PK, autoincrement (local to this DB; global IDs are packed). |
| rel_path        | TEXT    | Folder-relative path, forward slashes, UNIQUE.              |
| filename        | TEXT    | Basename (`IMG_1234.jpg`).                                  |
| ext             | TEXT    | Lowercase extension, no dot.                                |
| size_bytes      | INTEGER | From `fs::Metadata::len()`.                                 |
| mtime_ms        | INTEGER | Modification time in Unix ms.                               |
| width           | INTEGER | Nullable; from handler's technical read.                    |
| height          | INTEGER | Nullable.                                                   |
| content_hash    | TEXT    | Nullable; XXH3 128-bit hex digest.                          |
| taken_at        | INTEGER | Nullable; Unix ms from EXIF `DateTimeOriginal`.             |
| camera_make     | TEXT    | Nullable.                                                   |
| camera_model    | TEXT    | Nullable.                                                   |
| title           | TEXT    | Nullable; user-editable.                                    |
| imported_at     | INTEGER | Unix ms when the row was first inserted.                    |
| missing         | INTEGER | 0/1. 1 after a scan that failed to see the file.            |

**Indexes:** `idx_images_taken_at`, `idx_images_filename`, `idx_images_ext`.

### `tags`

| Column | Type    | Notes                                     |
| ------ | ------- | ----------------------------------------- |
| **id** | INTEGER | PK, autoincrement.                        |
| name   | TEXT    | UNIQUE COLLATE NOCASE.                    |

### `image_tags`

Many-to-many join.

| Column     | Type    | Notes                                          |
| ---------- | ------- | ---------------------------------------------- |
| image_id   | INTEGER | FK → `images(id)` ON DELETE CASCADE.           |
| tag_id     | INTEGER | FK → `tags(id)` ON DELETE CASCADE.             |
| **PK**     |         | (image_id, tag_id).                            |

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

- Contentless (`content=''`) — `rowid` is `images.id`; we join.
- `contentless_delete=1` — required for our rebuild-per-row strategy.
- Diacritics folded (`naive` = `naïve`).

## Global ID scheme

The IPC layer always exposes a packed *global* ID so the frontend
never sees the per-folder `images.id` directly:

```
gid = folder_id * 1_000_000_000 + local_id
```

Room for 9 million folders × 1 billion images each, all inside
JavaScript's `Number.MAX_SAFE_INTEGER`. Helper functions live in
`src-tauri/src/db/mod.rs` (`pack_global_id`, `unpack_global_id`).

## Relationships

```
registry.db                     <folder>/.magpie/library.db (attached as f<id>)
───────────                     ─────────────────────────────
library_folders                  images ─────────┐
   ▲ (id = folder_id)               ▲            │ (1..N)
   │                                │ (rowid=id) │
   │                                │            ▼
smart_collections                images_fts    image_tags
app_settings                                     ▲
                                                 │ (N..1)
                                                tags
```

## Migrations

Registry: freshly created on first launch, no migration history yet.

Library: `folder_meta.schema_version` is bumped when the schema
changes. Only version 1 exists today.

Legacy migration: on first launch after the redesign, if
`registry.db` is missing but a legacy central `library.db` is
present, `db::legacy_migration::migrate_legacy_central_db` splits
the central DB into per-folder DBs, populates `registry.db`, and
renames the legacy file to `library.db.migrated-<timestamp>` so
nothing is destroyed. See [Database redesign](./db-redesign.md#migration-from-the-legacy-central-db).
