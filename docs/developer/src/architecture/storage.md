# Data storage architecture

## Three layers

```
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 1: Source files (the source of truth)             │
   │                                                          │
   │  Photo.jpg  ←──  embedded XMP APP1 segment (JPEG only)   │
   │  Photo.xmp  ←──  sidecar XMP file (all formats)          │
   └──────────────────────────────────────────────────────────┘
                                ▲
                                │  read / write
                                ▼
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 2: The library index (cache; can be rebuilt)      │
   │                                                          │
   │  %APPDATA%\com.picorg.picorg\picorg.db                   │
   │  ├─ library_folders                                      │
   │  ├─ images                                               │
   │  ├─ tags + image_tags                                    │
   │  ├─ images_fts  (FTS5)                                   │
   │  └─ smart_collections                                    │
   └──────────────────────────────────────────────────────────┘
                                ▲
                                │  render / write via IPC
                                ▼
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 3: Thumbnails (derived; safe to delete)           │
   │                                                          │
   │  %APPDATA%\com.picorg.picorg\thumbs\<id>-<size>.webp     │
   └──────────────────────────────────────────────────────────┘
```

Anything a user considers "their data" lives in Layer 1. Layers 2
and 3 exist purely for speed and can be reconstructed at any time by
rescanning the library folders.

## Layer 1: source of truth (filesystem)

### Sidecar files

PicOrg follows the Lightroom convention for sidecar naming:

- `IMG_1234.jpg` → `IMG_1234.xmp`
- `IMG_1234.CR2` → `IMG_1234.xmp` (extension stripped)
- `IMG_1234.HEIC` → `IMG_1234.xmp`

Sidecars are written as UTF-8 XML with a BOM-prefixed XMP packet
wrapper (`<?xpacket …?>`). They contain the subset of XMP PicOrg
edits — `dc:title`, `xmp:Rating`, `dc:description`, `dc:subject` —
plus a Microsoft-friendly `MicrosoftPhoto:LastKeywordXMP` alias for
tags.

### Embedded XMP inside JPEG

For JPEG source files, PicOrg also injects the same XMP packet as
an APP1 segment right after SOI (matching Adobe's recommended
placement). Any pre-existing standard-XMP or ExtendedXMP APP1 is
removed first, so writes are replace-in-place, not append.

The rewrite is atomic: temp file next to the original, `fsync`,
rename. Details in [Metadata write path](../design/metadata-write.md).

### Precedence when both exist

When PicOrg reads a photo, `read_all` merges embedded and sidecar
XMP with **sidecar values taking precedence**. This matches what
Lightroom and digiKam do — the sidecar reflects the latest edits.

## Layer 2: the SQLite index

Location: `%APPDATA%\com.picorg.picorg\picorg.db`.

The database has six user tables plus `_migrations` and the FTS5
virtual table `images_fts`. Full schema in
[Database schema](../design/schema.md).

Key properties:

- **Single writer** via a `Mutex<Connection>`. All writes are
  transactional; a failure rolls back cleanly.
- **FTS5 in `contentless_delete=1` mode**. This is a SQLite 3.43+
  feature that lets us `DELETE FROM images_fts` before re-inserting
  a row — necessary because we rebuild an FTS row on every metadata
  update. See migration `0002_fts_contentless_delete` in
  `src-tauri/src/db/migrations.rs`.
- **`meta_read_at` / `meta_written_at`** columns on `images` are
  the pivot for FS synchronisation: if the sidecar's mtime is newer
  than `meta_read_at`, PicOrg re-reads the FS before serving a
  detail request.

## Layer 3: thumbnail cache

Location: `%APPDATA%\com.picorg.picorg\thumbs\`.

Format: WebP with quality 80, two sizes per image
(`<id>-small.webp`, `<id>-medium.webp`). Small is ~256 px on the
long edge, medium ~512 px.

Regeneration:

- On scan, if a thumbnail is missing or the source file's `mtime` is
  newer than the thumbnail's, PicOrg regenerates.
- On delete, `delete_thumbnails(cache_dir, image_id)` removes every
  size.
- The user can safely delete the whole `thumbs\` folder; PicOrg
  regenerates on next request.

## Volume assumptions

- Library folders can live on any local volume, including NTFS with
  long paths (`\\?\`-prefixed) and OneDrive-mounted trees.
- The database and cache always live on `%APPDATA%` — the OS default
  local storage. Users who want to move it to another volume should
  create a junction; PicOrg does not have a "move library" UI in v1.

## Backup story

Everything that matters is on the filesystem outside PicOrg's
control: your original photos plus the sidecar `.xmp` files. Back
those up and your library is safe. The SQLite database is regen-
erable and doesn't need to be in a backup — but including it means
a restore doesn't require a rescan.
