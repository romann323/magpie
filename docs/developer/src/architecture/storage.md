# Data storage architecture

## Three layers

```
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 1: Source files (the source of truth)             │
   │                                                          │
   │  Photo.jpg  ←──  embedded XMP APP1 segment                │
   │  Photo.png  ←──  embedded XMP iTXt chunk                  │
   │                                                          │
   │  Magpie NEVER creates a .xmp sidecar file next to a       │
   │  photo. Metadata lives inside the source file only.       │
   └──────────────────────────────────────────────────────────┘
                                ▲
                                │  read / write
                                ▼
   ┌──────────────────────────────────────────────────────────┐
   │  Layer 2: The library index (cache; can be rebuilt)      │
   │                                                          │
   │  %APPDATA%\com.magpie.app\library.db                   │
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
   │  %APPDATA%\com.magpie.app\thumbs\<id>-<size>.webp     │
   └──────────────────────────────────────────────────────────┘
```

Anything a user considers "their data" lives in Layer 1. Layers 2
and 3 exist purely for speed and can be reconstructed at any time by
rescanning the library folders.

## Layer 1: source of truth (filesystem)

### Embedded XMP inside the source file

For formats whose handler advertises `can_write_tags() == true`,
all user metadata (title + tags) is embedded directly into the
source file:

- **JPEG (`.jpg`, `.jpeg`)** — the XMP packet is injected as an
  APP1 segment right after SOI. Any pre-existing standard-XMP or
  ExtendedXMP APP1 is stripped first.
- **PNG (`.png`)** — the packet is stored as an `iTXt` chunk with
  keyword `XML:com.adobe.xmp`, inserted immediately after `IHDR`.
  CRC-32 is recomputed for the new chunk.
- **WebP (`.webp`)** — the packet is stored as a RIFF `XMP `
  chunk. Simple-form WebPs are converted to extended VP8X form on
  write if needed.
- **GIF (`.gif`, GIF89a only)** — the packet is stored in an
  Application Extension block using Adobe's XMP magic trailer.
  GIF87a files are rejected because the extension block was only
  added in GIF89a.

All rewrites are atomic: temp file next to the original,
`sync_all()`, rename. Details in
[Metadata write path](../design/metadata-write.md).

The XMP packet uses the same field mapping either way — `dc:title`
and `dc:subject`, plus a Microsoft-friendly
`MicrosoftPhoto:LastKeywordXMP` alias for tags so Windows Explorer's
*Tags* column populates. Any `xmp:Rating` or `dc:description`
already present is passed through unchanged (Magpie's UI doesn't
expose those fields but does not clobber them either).

### Read-only formats

For formats whose handler declares `can_write_tags() == false`
(TIFF, HEIC, PDF, video, camera RAW, …),
`handler.write_user` returns `AppError::MetadataWrite` with a
message naming the offending extension. Magpie does **not**
silently fall back to writing a sidecar. The UI shows a note that
the tag is stored in Magpie's library only, but the tag is still
persisted in the DB so filtering / searching keeps working.

### Legacy sidecar files

Older Magpie versions and other tools (Lightroom, digiKam) may have
left `Photo.xmp` files next to some images. Magpie's reader still
picks these up on scan so users don't lose data. On the first
successful save into the source file, `write_metadata_to_source`
best-effort-deletes the leftover sidecar — after that the photo is
the sole source of truth.

Sidecar path detection follows the Lightroom convention:
`IMG_1234.jpg → IMG_1234.xmp` (strip original extension). The
alternative `IMG_1234.jpg.xmp` (digiKam) form is not read.

### Precedence when both exist (read path only)

If both the file's embedded XMP and a legacy sidecar exist,
`read_all` merges embedded first and lets the sidecar overwrite,
so the sidecar wins. This matches what Lightroom does and means an
existing sidecar authored elsewhere still takes effect until Magpie
saves and cleans it up.

## Layer 2: the SQLite index

Location: `%APPDATA%\com.magpie.app\library.db`.

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
  the pivot for FS synchronisation: if the source file's (or any
  legacy sidecar's) mtime is newer than `meta_read_at`, Magpie re-
  reads the FS before serving a detail request.

## Layer 3: thumbnail cache

Location: `%APPDATA%\com.magpie.app\thumbs\`.

Format: WebP with quality 80, two sizes per image
(`<id>-small.webp`, `<id>-medium.webp`). Small is ~256 px on the
long edge, medium ~512 px.

Regeneration:

- On scan, if a thumbnail is missing or the source file's `mtime` is
  newer than the thumbnail's, Magpie regenerates.
- On delete, `delete_thumbnails(cache_dir, image_id)` removes every
  size.
- The user can safely delete the whole `thumbs\` folder; Magpie
  regenerates on next request.

## Volume assumptions

- Library folders can live on any local volume, including NTFS with
  long paths (`\\?\`-prefixed) and OneDrive-mounted trees.
- The database and cache always live on `%APPDATA%` — the OS default
  local storage. Users who want to move it to another volume should
  create a junction; Magpie does not have a "move library" UI in v1.

## Backup story

Everything that matters is on the filesystem outside Magpie's
control: your original photos, with the metadata embedded inside
them. Back those up and your library is safe. The SQLite database
is regenerable and doesn't need to be in a backup — but including
it means a restore doesn't require a rescan.
