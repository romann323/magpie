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
   │  Central: %APPDATA%\com.magpie.app\registry.db           │
   │  ├─ library_folders                                      │
   │  ├─ smart_collections                                    │
   │  └─ app_settings                                         │
   │                                                          │
   │  Per-folder: <folder>\.magpie\library.db                 │
   │  ├─ folder_meta                                          │
   │  ├─ images (rel_path, title, tags via image_tags)        │
   │  ├─ tags + image_tags                                    │
   │  └─ images_fts (FTS5)                                    │
   └──────────────────────────────────────────────────────────┘
                                ▲
                                │  render / write via IPC
                                ▼
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 3: Thumbnails (derived; safe to delete)           │
   │                                                          │
   │  %APPDATA%\com.magpie.app\thumbs\<gid>-<size>.webp       │
   └──────────────────────────────────────────────────────────┘
```

Anything a user considers "their tag data" lives in Layer 2 —
specifically inside the per-folder `library.db`. Layer 3 exists
purely for speed and can be regenerated at any time by rescanning
the library folders.

See [Database redesign](../design/db-redesign.md) for the deeper
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
older Magpie), those tags are imported into the per-folder DB at
first-scan time. Older Magpie versions used `.xmp` sidecar files;
`core::metadata::sidecar` still reads those on import for backward
compatibility, but no new sidecars are ever written.

## Layer 2: SQLite (two-tier)

- **Registry DB** — `%APPDATA%\com.magpie.app\registry.db`. Small.
  Owns the list of registered folders, smart collections, and
  app-wide settings. Kept open for the app lifetime with every
  registered library `ATTACH DATABASE`-ed as `f<id>` for cross-folder
  queries.
- **Library DB** — `<folder>/.magpie/library.db`, one per folder.
  Owns every image row (paths stored folder-relative) and the folder's
  own tag namespace + FTS index. Portable: copying the folder to
  another disk carries the tags along.

Key properties:

- **Single writer per folder** via a `Mutex<Connection>` inside
  `LibraryDb`. Different folders write in parallel.
- **WAL mode** enabled per DB so readers (search, thumbnails,
  DetailsPanel loads) never block writers.
- **FTS5 in `contentless_delete=1` mode** — required for our
  rebuild-per-row strategy on metadata edits.
- **Global IDs are packed** at the IPC boundary
  (`folder_id * 1_000_000_000 + local_id`) so the frontend can treat
  every image as a single-integer entity.

Full schema in [Database schema](../design/schema.md).

## Layer 3: thumbnail cache

Location: `%APPDATA%\com.magpie.app\thumbs\`.

Format: WebP with quality 80, two sizes per image
(`<gid>-small.webp`, `<gid>-medium.webp`). Small is ~256 px on the
long edge, medium ~512 px. `<gid>` is the packed global ID so
thumbs from different folders don't collide.

Regeneration:

- On scan, if a thumbnail is missing or the source file's `mtime` is
  newer than the thumbnail's, Magpie regenerates.
- On delete, `delete_thumbnails(cache_dir, gid)` removes every
  size.
- The user can safely delete the whole `thumbs\` folder; Magpie
  regenerates on next request.

## Volume assumptions

- Library folders can live on any local volume, including NTFS with
  long paths (`\\?\`-prefixed).
- On cloud-synced folders (OneDrive, Dropbox, Google Drive, iCloud)
  or UNC network shares, Magpie warns via
  `check_folder_sync_risk` before the folder is added — see
  [Database redesign § Sync-location warning](../design/db-redesign.md#sync-location-warning).
- The registry DB and thumbnail cache always live under `%APPDATA%`.

## Backup story

Because tags now live **inside each folder** (in `.magpie/library.db`),
a plain backup of the photo folder captures the tag data too. The
central `registry.db` only records which folders are registered;
nothing user-critical is lost if it's wiped (the user just re-adds
each folder).
