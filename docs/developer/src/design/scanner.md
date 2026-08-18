# Scanner algorithm

## Objective

Given a library folder path, populate the `images` table with a
row per supported image file, extract metadata, generate
thumbnails, and keep the DB in sync with the filesystem on
subsequent runs — all while staying responsive.

## Pipeline

```
   walk()  →  filter()  →  diff()  →  extract()  →  upsert()  →  thumbnail()
      │           │            │            │            │             │
   jwalk::   ext-in-       mtime          rayon         DB           thumb
   WalkDir   IMAGE_EXTS   match?         parallel     serialised    pipeline
    par      ↳ skip       ↳ skip                       (mutex)
    walk    others        else read
```

Progress events (`picorg://scan { folder_id, done, total, current }`)
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

For each kept entry, PicOrg looks up the path in `images`:

```
SELECT id, mtime_ms FROM images WHERE path = ?1
```

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
3. **EXIF** — `kamadak-exif::Reader::read_from_container` for
   `taken_at`, `camera_make`, `camera_model`, `width`, `height`.
4. **XMP** — `metadata::read::read_all(path)` for `title`,
   `rating`, `comment`, `tags`.

Any per-file failure is logged (`WARN`) and doesn't abort the
folder scan. The file gets a row with whatever metadata succeeded;
missing fields are left NULL.

## Stage 5 — upsert

The DB is the single writer. To amortise transaction overhead,
extracted rows are pushed to a bounded channel; a single "writer
task" drains the channel and commits batches of up to 128 rows in
one transaction:

```rust
let tx = conn.transaction()?;
for row in batch { upsert_image(&tx, &row)?; }
tx.commit()?;
```

The `upsert_image` SQL is:

```sql
INSERT INTO images (path, folder_id, filename, ext, size_bytes, mtime_ms,
                    width, height, content_hash, taken_at,
                    camera_make, camera_model, title, rating, comment,
                    meta_read_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(path) DO UPDATE SET
    size_bytes    = excluded.size_bytes,
    mtime_ms      = excluded.mtime_ms,
    width         = excluded.width,
    height        = excluded.height,
    content_hash  = excluded.content_hash,
    taken_at      = excluded.taken_at,
    camera_make   = excluded.camera_make,
    camera_model  = excluded.camera_model,
    title         = excluded.title,
    rating        = excluded.rating,
    comment       = excluded.comment,
    meta_read_at  = excluded.meta_read_at;
```

After the row lands, tags are reconciled: everything in `image_tags`
for this image is deleted, then re-inserted from the fresh tag list.
Both happen in the same transaction as the image upsert.

The FTS5 row is rebuilt via `rebuild_fts_row_tx` at the very end.

## Stage 6 — thumbnails

Once a row is in the DB, the scanner enqueues a thumbnail task for
that image id. Thumbnails run on the same rayon pool but with lower
priority so they don't starve extraction.

`ensure_thumbnails(cache_dir, src_path, image_id)`:

1. For each size (small, medium):
   - Path = `cache_dir / "<id>-<size>.webp"`.
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
