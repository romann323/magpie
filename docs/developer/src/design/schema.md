# Database schema

The database is a single SQLite file at
`%APPDATA%\com.picorg.picorg\picorg.db`. Schema definition lives in
`src-tauri/src/db/migrations.rs`.

## Tables

### `library_folders`

Every folder the user has added to the library.

| Column        | Type    | Notes                                       |
| ------------- | ------- | ------------------------------------------- |
| **id**        | INTEGER | PK, autoincrement.                          |
| path          | TEXT    | Absolute, canonical, UNIQUE.                |
| added_at      | INTEGER | Unix ms.                                    |
| last_scan_at  | INTEGER | Unix ms; NULL before first scan finishes.   |

### `images`

One row per image file found by the scanner.

| Column          | Type    | Notes                                                    |
| --------------- | ------- | -------------------------------------------------------- |
| **id**          | INTEGER | PK, autoincrement.                                       |
| folder_id       | INTEGER | FK → `library_folders(id)` ON DELETE CASCADE.            |
| path            | TEXT    | Absolute path, UNIQUE.                                   |
| filename        | TEXT    | Basename (`IMG_1234.jpg`).                               |
| ext             | TEXT    | Lowercase extension, no dot (`jpg`, `heic`, …).          |
| size_bytes      | INTEGER | From `fs::Metadata::len()`.                              |
| mtime_ms        | INTEGER | Modification time in Unix ms.                            |
| width           | INTEGER | Nullable; from EXIF or image decode.                     |
| height          | INTEGER | Nullable.                                                |
| content_hash    | TEXT    | Nullable; XXH3 128-bit hex digest.                       |
| taken_at        | INTEGER | Nullable; Unix ms from EXIF `DateTimeOriginal`.          |
| camera_make     | TEXT    | Nullable.                                                |
| camera_model    | TEXT    | Nullable.                                                |
| title           | TEXT    | Nullable; user-editable via UI.                          |
| rating          | INTEGER | CHECK: NULL or 0..=5.                                    |
| comment         | TEXT    | Nullable.                                                |
| meta_written_at | INTEGER | Nullable; last time PicOrg wrote XMP for this photo.     |
| meta_read_at    | INTEGER | Nullable; last time PicOrg read XMP from disk into DB.   |
| missing         | INTEGER | 0/1 flag for "file expected but not found" (unused v1).  |

**Indexes:**

- `idx_images_folder` (folder_id)
- `idx_images_taken_at` (taken_at)
- `idx_images_rating` (rating)
- `idx_images_filename` (filename)

### `tags`

The tag vocabulary.

| Column    | Type    | Notes                                     |
| --------- | ------- | ----------------------------------------- |
| **id**    | INTEGER | PK, autoincrement.                        |
| name      | TEXT    | UNIQUE COLLATE NOCASE.                    |

### `image_tags`

Many-to-many join between `images` and `tags`.

| Column     | Type    | Notes                                          |
| ---------- | ------- | ---------------------------------------------- |
| image_id   | INTEGER | FK → `images(id)` ON DELETE CASCADE.           |
| tag_id     | INTEGER | FK → `tags(id)` ON DELETE CASCADE.             |
| **PK**     |         | (image_id, tag_id).                            |

**Indexes:**

- `idx_image_tags_tag` (tag_id) — accelerates "photos with tag X".

### `images_fts` (virtual, FTS5)

Full-text search index over four columns.

```sql
CREATE VIRTUAL TABLE images_fts USING fts5(
    title, comment, filename, tags,
    content='',
    contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2'
);
```

Key attributes:

- **Contentless** (`content=''`): the FTS table doesn't store the
  original text; queries return `rowid` (= `images.id`) and we join.
- **`contentless_delete=1`** (SQLite 3.43+): lets us `DELETE FROM
  images_fts` before re-inserting a row. Without it,
  `rebuild_fts_row_tx` fails and rolls back every metadata write.
  See migration `0002_fts_contentless_delete`.
- **Diacritics removed at level 2**: `naive` and `naïve` are the same
  token.

### `smart_collections`

Skeleton in v1 — data model exists, UI to come.

| Column     | Type    | Notes                                     |
| ---------- | ------- | ----------------------------------------- |
| **id**     | INTEGER | PK, autoincrement.                        |
| name       | TEXT    |                                           |
| filter     | TEXT    | JSON-serialised `ImageFilter`.            |
| sort_order | INTEGER |                                           |

### `_migrations`

Housekeeping.

| Column     | Type    | Notes                                    |
| ---------- | ------- | ---------------------------------------- |
| **name**   | TEXT    | PK; migration name (e.g. `0001_init`).   |
| applied_at | INTEGER | Unix ms when the migration ran.          |

## Relationships

```
library_folders ─┐
                 │ (1..N)
                 ▼
              images ────────┐
                 ▲           │ (1..N)
                 │ (rowid=id)│
                 │           ▼
              images_fts   image_tags
                             ▲
                             │ (N..1)
                             │
                            tags
```

## Migrations

| Name                          | Effect                                                       |
| ----------------------------- | ------------------------------------------------------------ |
| `0001_init`                   | Full schema above (without `contentless_delete`).            |
| `0002_fts_contentless_delete` | Drop + recreate `images_fts` with `contentless_delete=1`;    |
|                               | repopulate from live data; blank `meta_read_at` to force     |
|                               | FS re-read.                                                  |

Each migration is applied atomically inside a transaction and
recorded in `_migrations` on success.
