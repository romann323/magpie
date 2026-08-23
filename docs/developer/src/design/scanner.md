# Scanner algorithm

## Objective

Given a library folder path, populate the folder's per-folder
`library.db` (`<folder>/.magpie/library.db`) with a row per
supported file, extract metadata, generate thumbnails, and keep
the DB in sync with the filesystem on subsequent runs — all while
staying responsive.

Every path stored in the DB is **folder-relative** (forward
slashes). The scanner joins root + relative path when it needs the
actual filesystem location.

## Pipeline

```
   walk()  →  filter()  →  diff()  →  extract()  →  upsert()  →  thumbnail()
      │           │            │            │            │             │
   jwalk::   ext-in-       mtime          rayon         DB           thumb
   WalkDir   IMAGE_EXTS   match?         parallel     serialised    pipeline
    par      ↳ skip       ↳ skip                       (mutex)
    walk    others        else read
```

Progress events (`app://scan { folder_id, done, total, current }`)
are emitted every N files (default 20).

## Stage 1 — walk

`jwalk::WalkDir::new(root).parallelism(Parallelism::RayonNewPool(n))`
walks the folder tree in parallel across all cores, streaming
`DirEntry` values. Symlinks are followed (with cycle detection);
hidden files are respected per OS convention.

## Stage 2 — filter

An entry is kept iff:

- It's a regular file (not a directory, socket, or device).
- Its lowercase extension is in `core::IMAGE_EXTS`:
  `jpg jpeg jpe jfif jif png gif bmp tif tiff webp heic heif`
  `cr2 cr3 nef arw dng raf orf pef x3f`.
- Its filename doesn't start with `.` (dotfiles).

Discarded entries increment a counter used for the progress
denominator but otherwise do no work.

## Stage 3 — diff

For each kept entry, Magpie looks up the *folder-relative* path in
the folder's `images` table:

```
SELECT id, mtime_ms FROM images WHERE rel_path = ?1
```

`.magpie/library.db` itself (and its WAL / SHM sidecar files) is
excluded from the walk so the scanner doesn't try to import its own
storage.

- **New file** (no row): full extract required.
- **Existing, mtime unchanged**: skip.
- **Existing, mtime changed**: partial re-extract
  (metadata + hash), leaving other columns intact.

The lookup is O(log n) thanks to the UNIQUE index on `images.path`.

## Stage 4 — extract

For each file that needs work, the following runs on a rayon
worker:

1. **Stat** — `fs::metadata(path)` for `size_bytes` and `mtime_ms`.
2. **Content hash** — stream the file through `XXH3_128`. On an
   SSD this is disk-bound at ~2 GB/s; on HDD it's the bottleneck.
3. **Handler lookup** — `registry.for_ext(ext)` picks the
   `FormatHandler` for the file's extension.
4. **Technical read** — the handler's `read_technical` produces
   ordered key/value pairs; those we recognise fold into
   `taken_at`, `camera_make`, `camera_model`, `width`, `height`.
5. **User meta** — `metadata::read::read_all` combines the
   handler's `read_user` with any legacy sidecar for `title` and
   `tags`.

Any per-file failure is logged (`WARN`) and doesn't abort the
folder scan. The file gets a row with whatever metadata succeeded;
missing fields are left NULL.

## Stage 5 — upsert

The folder's `library.db` is the single writer for this folder;
different folders write in parallel. Extracted rows are pushed to a
bounded channel and a writer task commits batches of up to 128 rows
per transaction:

```rust
let tx = conn.transaction()?;
for row in batch { library::upsert_image(&tx, &row)?; }
tx.commit()?;
```

The upsert SQL uses `rel_path` (folder-relative) as the conflict key:

```sql
INSERT INTO images (rel_path, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at,
                    camera_make, camera_model, title, imported_at, missing)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
ON CONFLICT(rel_path) DO UPDATE SET
    size_bytes    = excluded.size_bytes,
    mtime_ms      = excluded.mtime_ms,
    width         = excluded.width,
    height        = excluded.height,
    content_hash  = excluded.content_hash,
    taken_at      = excluded.taken_at,
    camera_make   = excluded.camera_make,
    camera_model  = excluded.camera_model,
    title         = excluded.title,
    missing       = 0;
```

After the row lands, tags are reconciled: everything in `image_tags`
for this image is deleted, then re-inserted from the fresh tag list.
Both happen in the same transaction as the image upsert.

The FTS5 row is rebuilt via `rebuild_fts_row_tx` at the very end.

## Stage 6 — thumbnails

Once a row is in the DB, the scanner enqueues a thumbnail task for
that image id. Thumbnails run on the same rayon pool but with lower
priority so they don't starve extraction.

`ensure_thumbnails(cache_dir, src_path, gid)`:

1. For each size (small, medium):
   - Path = `cache_dir / "<gid>-<size>.webp"` where `gid` is the
     *packed global ID* (`pack_global_id(folder_id, local_id)`).
   - Skip if the thumbnail's mtime ≥ the source's mtime.
2. Decode the source with `image::open`.
3. Resize with `fast_image_resize::Resizer` (Lanczos3, SIMD).
4. Encode with `webp::Encoder::new(&image).encode(80)`.
5. Write bytes atomically (temp + rename).

Failures at any stage are logged and don't affect the DB row.

## Handling deletions

If a file that was in the DB is no longer found on disk during a
scan, the scanner does **not** delete the row automatically — the
`missing` flag in the `images` table exists to mark it (v1: not
exposed in UI). A future contribution can add an explicit
"Clean up missing photos" UI action.

## Incremental performance

The diff step is what makes rescans fast:

- 50 000 photos, no changes: `~2 seconds` (mostly the DB lookup and
  a stat call per file).
- 50 000 photos, 100 new: `~5 seconds` (mostly hashing + XMP read
  for the 100).
- 50 000 photos, cold cache (thumbnails missing): `~2 minutes`
  (bounded by encoding).
