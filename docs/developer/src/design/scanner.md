# Scanner algorithm

## Objective

Given a library folder path, populate `magpie.db` with a row per
supported file, extract metadata, generate thumbnails, and keep the
DB in sync with the filesystem on subsequent runs — all while
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
the `images` table:

```
SELECT id, mtime_ms FROM images
 WHERE folder_id = ?1 AND rel_path = ?2
```

Leftover `.magpie/library.db` files from a failed migration off the
previous per-folder layout are excluded from the walk so the scanner
doesn't try to import them.

- **New file** (no row): full extract required.
- **Existing, mtime unchanged**: skip.
- **Existing, mtime changed**: partial re-extract
  (metadata + hash), leaving other columns intact.

The lookup is O(log n) thanks to the UNIQUE index on
`(folder_id, rel_path)`.

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

The central `magpie.db` is the single writer. Scanner tasks take
short borrows of the `Db` mutex — one upsert per statement — so
scan work interleaves with UI queries without long-lived locks.

The upsert uses `(folder_id, rel_path)` as the conflict key:

```sql
INSERT INTO images (folder_id, rel_path, filename, ext,
                    size_bytes, mtime_ms, imported_at, missing)
VALUES (?, ?, ?, ?, ?, ?, ?, 0)
ON CONFLICT(folder_id, rel_path) DO UPDATE SET
    filename    = excluded.filename,
    ext         = excluded.ext,
    size_bytes  = excluded.size_bytes,
    mtime_ms    = excluded.mtime_ms,
    missing     = 0;
```

After the row lands, tags are reconciled by `set_image_meta`.
Auto tags are **additive-only**: each name the file itself reports
is inserted with `source = 'auto'` **only when the image doesn't
already carry that name in either source**. Nothing is deleted, so
auto tags that vanished from the file's metadata since the last scan
stay in the DB, and user tags typed inside Magpie always survive.
See [Schema › image_tags](./schema.md#image_tags) for the storage
model. All of this happens in the same transaction as the image
upsert.

The FTS5 row is rebuilt via `rebuild_fts_row_tx` at the very end.

## Stage 6 — thumbnails

Once a row is in the DB, the scanner enqueues a thumbnail task for
that image id. Thumbnails run on the same rayon pool but with lower
priority so they don't starve extraction.

`ensure_thumbnails(cache_dir, src_path, image_id)`:

1. For each size (small, medium):
   - Path = `cache_dir / "<image_id>-<size>.webp"`. `image_id` is
     the plain `images.id` primary key — globally unique because
     Magpie has a single central DB.
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

## Stage 7 — Auto-tag pass (opt-in)

When **Settings → Auto-tag photos** is on (`AppSettings.aiAutoTag`),
`commands::library::add_library_folder` chains an automatic AI
classification pass onto the tail of a successful scan:

```
scanner::scan_folder(...)  →  auto_tag::tag_folder(...)
```

Structurally identical to the scanner:

- One `spawn_blocking` task per image, bounded by a
  `Semaphore::new(cpus)`.
- Progress emitted on the `app://auto-tag` event as an
  [`AutoTagProgress`](./commands.md#events) payload every 5 images
  (and once on start / finish).
- A single `tokio::sync::Mutex<()>` on `AppServices.auto_tag_gate`
  serialises AI passes across folders — if the user drops several
  folders in quick succession they queue up FIFO instead of
  competing for CPU. Filesystem scans stay parallel.

Per-image loop (`core::auto_tag::tag_one`):

1. Enumerate candidates via
   `queries::list_auto_tag_candidates(folder_id)`. Each candidate
   carries a "fingerprint" — the row's `content_hash` if the
   handler wrote one, else `mtime_ms.to_string()`.
2. If `ai_tag_hash == fingerprint`, skip — the image hasn't
   changed since the last successful pass. Counted as `skipped`
   in the progress payload.
3. Ensure the small thumbnail exists (reuse the scanner's
   `thumbnail::ensure_thumbnails`); if the format isn't decodable
   by the `image` crate, mark the row as tagged with zero
   suggestions so we don't try again next run.
4. Read the thumbnail bytes and call
   `ImageClassifier::classify(&bytes)`. In production the trait
   is implemented by `core::auto_tag::clip_classifier::ClipClassifier`,
   which runs OpenAI **CLIP-ViT-B/32** on the CPU via
   [`candle`](https://github.com/huggingface/candle) and cosine-ranks
   the resulting image embedding against a pre-computed matrix of
   text embeddings for the ~1 000-word bundled photo vocabulary
   (see [`core/auto_tag/`](./backend.md#coreauto_tag)). Tests use
   the deterministic `MockClassifier` in
   `core::auto_tag::classifier.rs` — swap in an `Arc<dyn
   ImageClassifier>` at the `tag_folder_with` entry point.
   `tag_folder` refuses to spawn the classifier when the CLIP model
   files aren't on disk — it emits one "finished with error"
   progress event instead of running the pass. The
   `AppServices.auto_tag_gate` also protects the model download
   against a concurrent auto-tag pass.
5. Filter suggestions by `classifier.min_confidence()`, sort by
   descending confidence, cap at `classifier.max_tags_per_image()`.
6. Attach the surviving names via
   `queries::add_auto_tags_for_image(image_id, names)`, which
   writes them as `'auto'`-source tags in a short transaction and
   rebuilds the FTS row. Names the image already carries (in
   either source) are a no-op, so the row count stays sane on
   rerun and any tag the user has typed themselves is left alone.
   The tags therefore render under the read-only **Automatic
   tags** section in the details panel, next to XMP-derived tags.
7. `queries::mark_image_ai_tagged(image_id, fingerprint, now_ms)`
   stamps `ai_tagged_at` and `ai_tag_hash` on the row.

Auto-tag never runs during plain `rescan_folder` or `rescan_all`
today — only on the very first scan of a newly-added folder. A
follow-up can add an explicit "run AI on this folder now" command
if we need it.

## Incremental performance

The diff step is what makes rescans fast:

- 50 000 photos, no changes: `~2 seconds` (mostly the DB lookup and
  a stat call per file).
- 50 000 photos, 100 new: `~5 seconds` (mostly hashing + XMP read
  for the 100).
- 50 000 photos, cold cache (thumbnails missing): `~2 minutes`
  (bounded by encoding).
