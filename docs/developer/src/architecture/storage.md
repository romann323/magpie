# Data storage architecture

## Three layers

```
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 1: Source files (untouched)                       │
   │                                                          │
   │  Photo.jpg, Photo.png, Photo.arw, video.mp4, ...         │
   │                                                          │
   │  Magpie NEVER modifies these bytes. Not the pixels, not  │
   │  the XMP, not the Windows Shell property store. Titles   │
   │  and tags live in Layer 2 instead.                       │
   └──────────────────────────────────────────────────────────┘
                                ▲
                                │  read-only on scan
                                ▼
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 2: SQLite (source of truth for user metadata)     │
   │                                                          │
   │  %APPDATA%\com.magpie.app\magpie.db                      │
   │  ├─ library_folders                                      │
   │  ├─ images (folder_id, rel_path, title, ...)             │
   │  ├─ tags + image_tags (image_tags carries a source col) │
   │  ├─ images_fts (FTS5)                                    │
   │  ├─ smart_collections                                    │
   │  ├─ app_settings                                         │
   │  └─ schema_meta                                          │
   └──────────────────────────────────────────────────────────┘
                                ▲
                                │  render / write via IPC
                                ▼
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 3: Thumbnails (derived; safe to delete)           │
   │                                                          │
   │  %APPDATA%\com.magpie.app\thumbs\<id>-<size>.webp        │
   └──────────────────────────────────────────────────────────┘
```

Anything a user considers "their tag data" lives in Layer 2. Layer 3
exists purely for speed and can be regenerated at any time by
rescanning the library folders.

See [Database design](../design/db-redesign.md) for the deeper
rationale.

## Layer 1: source files (read-only)

Magpie never modifies source files. The scanner reads:

- File metadata (size, mtime) via `std::fs::metadata`.
- Format-specific *technical* metadata (dimensions, EXIF, camera,
  duration, page count) via each `FormatHandler::read_technical`.
- Format-specific *user* metadata (title, tags) via
  `FormatHandler::read_user`, and additionally the Windows Shell
  property store for formats that don't natively parse (RAW, HEIC,
  MP4, PDF, …).

These reads happen exactly once per file at the first scan (and
again if the file's mtime moves forward), then the DB is
authoritative.

### Existing sidecars and embedded XMP

If the folder already contains XMP-tagged JPEGs (from Lightroom or
older Magpie), those tags are imported into `magpie.db` at
first-scan time. Older Magpie versions used `.xmp` sidecar files;
`core::metadata::sidecar` still reads those on import for backward
compatibility, but no new sidecars are ever written.

## Layer 2: single central SQLite

One file: `%APPDATA%\com.magpie.app\magpie.db`.

Key properties:

- **Single writer** via a `Mutex<Connection>` inside `Db`. Held only
  for the duration of a single query or a short transaction.
- **WAL mode** so readers (search, thumbnails, DetailsPanel loads,
  external DB Browser sessions) never block the writer.
- **Foreign keys with cascade** — deleting a folder wipes its
  images, which wipes their `image_tags` and FTS rows.
- **FTS5 in `contentless_delete=1` mode** — required for our
  rebuild-per-row strategy on metadata edits.
- **Plain autoincrement image IDs** — `images.id` is globally unique
  because there's only one DB. The IPC layer forwards the ID
  verbatim to the frontend.

Full schema in [Database schema](../design/schema.md).

## Layer 3: thumbnail cache

Location: `%APPDATA%\com.magpie.app\thumbs\`.

Format: WebP with quality 80, two sizes per image
(`<id>-small.webp`, `<id>-medium.webp`). Small is ~256 px on the
long edge, medium ~512 px. `<id>` is `images.id`, which is unique
across the whole app.

Regeneration:

- On scan, if a thumbnail is missing or the source file's `mtime` is
  newer than the thumbnail's, Magpie regenerates.
- On delete, `delete_thumbnails(cache_dir, id)` removes every size.
- The user can safely delete the whole `thumbs\` folder; Magpie
  regenerates on next request.

## Volume assumptions

- Library folders can live on any local volume, including NTFS with
  long paths (`\\?\`-prefixed).
- Cloud-synced folders (OneDrive, Dropbox, Google Drive, iCloud) and
  UNC network shares are fine as library roots: the DB itself is in
  `%APPDATA%` and isn't affected by whatever the sync client does to
  the photos.
- The central DB and thumbnail cache always live under `%APPDATA%`.

## Backup story

`magpie.db` is a single file. Back it up (or the whole
`%APPDATA%\com.magpie.app\` directory to include thumbs) and you've
backed up every tag, title, smart collection, and setting Magpie
knows about.

The photo folders themselves don't carry tag data — copying a folder
to another disk copies only the pixels. To bring tags along you
either copy `magpie.db` too, or re-add the folder on the target PC
and let the first scan import whatever the files already embed.
