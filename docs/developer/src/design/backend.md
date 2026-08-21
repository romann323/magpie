# Backend modules

## `lib.rs` — app entry

Responsibilities:

- Initialise file-based logging (`init_logging`).
- Build `AppServices { db, cache_dir, … }` under `Arc`.
- Register every Tauri plugin (`dialog`, `opener`) and command
  handler in `invoke_handler!`.
- Manage the app run loop.

Notable pitfalls:

- `init_logging` **must not** call `tracing_subscriber::fmt().init()`
  if a Tauri plugin (like the removed `tauri-plugin-log`) already
  set a global subscriber. The prior implementation crashed with
  `PluginInitialization("log", "attempted to set a logger after…")`;
  the current version owns the logger outright.

## `types.rs` — IPC types

- `ImageSummary`, `ImageDetails` — full and abridged views of an
  image row.
- `MetadataPatch` — the "what to change" payload for
  `update_image_metadata` and `batch_update_metadata`.
- `double_option` module — custom Serde deserializer for
  `Option<Option<T>>`. Distinguishes "field missing" from
  "field explicitly null" on the wire.

Every struct uses `#[serde(rename_all = "camelCase")]` so JSON keys
are camelCase but Rust fields stay snake_case.

## `error.rs`

`AppError` enum (via `thiserror`):

- `Io(std::io::Error)` — filesystem errors.
- `Db(rusqlite::Error)` — DB errors.
- `MetadataRead(String)`, `MetadataWrite(String)` — user-facing
  metadata failures.
- `Scan(String)` — scanner-specific issues.
- `Internal(String)` — anything else.

All commands return `AppResult<T>` (= `Result<T, AppError>`).
The `Display` impl for `AppError` is safe to surface to the user
(no absolute paths in strings without context).

## `db/mod.rs`

Thin wrapper around `rusqlite::Connection`:

```rust
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> { … }

    pub fn with_conn<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&Connection) -> AppResult<T>,
    { … }
}
```

The single-mutex approach keeps SQLite calls simple and avoids
connection pooling. WAL mode is enabled on open, so concurrent
readers don't stall writers.

## `db/migrations.rs`

Two migrations in the ordered `MIGRATIONS` slice:

1. `0001_init` — full schema (see [Database schema](./schema.md)).
2. `0002_fts_contentless_delete` — recreate `images_fts` with
   `contentless_delete=1` and blank out `meta_read_at` so the FS
   sync path re-runs for every image.

Migrations run at startup inside `run(conn)`. Each is applied
atomically and recorded in the `_migrations` table.

## `db/queries.rs`

The functional heart of the DB layer. Highlights:

| Function                          | Purpose                                                |
| --------------------------------- | ------------------------------------------------------ |
| `apply_metadata_patch(db, id, p)` | Apply a `MetadataPatch` to an image in a single txn.   |
| `get_image(db, id)`               | Fetch full `ImageDetails` including tags.              |
| `query_images(db, filter, sort, page)` | Paginated grid query with sidebar filters + FTS.  |
| `list_tags(db, prefix)`           | Tags with counts, optionally prefix-filtered.          |
| `rename_tag(db, old, new)`        | Global rename in one txn (including FTS rebuild).      |
| `delete_tag(db, name)`            | Global delete of a tag.                                |
| `get_image_paths(db, ids)`        | Bulk fetch of paths for delete.                        |
| `delete_image_rows(db, ids)`      | Bulk row deletion after files are gone.                |
| `resync_user_meta_from_fs(db, id, meta)` | Sync FS-read metadata into DB, not touching     |
|                                   | scan-derived fields like size/mtime.                   |
| `set_meta_read_at_now(db, id)`    | Timestamp helper.                                      |
| `set_meta_written_at(db, id, t)`  | Timestamp helper.                                      |

Every mutation goes through a `Transaction` obtained via
`conn.transaction()?`, and every one calls `rebuild_fts_row_tx` at
the end to keep the search index in sync.

## `core/scanner.rs`

Pipeline for `add_library_folder` and `rescan_*`:

1. **Walk.** `jwalk::WalkDir` traverses the folder in parallel.
2. **Filter.** Extensions checked against `IMAGE_EXTS` (see
   `core/mod.rs`).
3. **Diff.** For each file, look up by path in `images`; skip if
   `mtime_ms` matches.
4. **Extract.** In parallel via `rayon`: EXIF read, XMP read, content
   hash (`XXH3`).
5. **Upsert.** DB writes are serialised via the single connection
   mutex; scanner buffers batches to amortise txn overhead.
6. **Thumbnails.** Enqueue small + medium.
7. **Progress.** Emit `app://scan { done, total, current }`.

Scanning is bounded by disk read speed on cold cache and by CPU on
warm cache.

## `core/thumbnail.rs`

```rust
pub fn ensure_thumbnails(cache_dir: &Path, src: &Path, image_id: i64) -> Result<()>;
pub fn thumb_path(cache_dir: &Path, image_id: i64, size: ThumbSize) -> PathBuf;
pub fn delete_thumbnails(cache_dir: &Path, image_id: i64);
```

Decode via `image::open`, resize via `fast_image_resize` (SIMD Lanczos3),
encode via `webp::Encoder` at quality 80.

## `core/metadata/read.rs`

`read_all(path) -> ImageMeta` runs the read pipeline:

1. `exif::Reader::read_from_container` for taken time + camera fields.
2. `xmp::extract_embedded_xmp(path)` for embedded XMP (JPEG APP1
   or PNG iTXt).
3. Read any legacy `<image>.xmp` sidecar for backward compatibility.
4. Merge sidecar-over-embedded via `apply_user_meta` (sidecar wins
   for user metadata so a Lightroom-authored `.xmp` still takes
   effect on first scan).

Non-fatal errors are collected into a per-file warning log; the
scanner continues on the next file.

## `core/formats/`

The format handler framework. Each supported extension is owned by
one `FormatHandler` implementation:

- `mod.rs` — declares the `FormatHandler` trait, `TechnicalMeta`,
  `UserMeta`, `FormatKind`, and the `FormatRegistry` that lives on
  `AppServices`.
- `xmp_packet.rs` — a hand-written streaming XMP reader/writer.
  Extracts and rebuilds packets containing title, description,
  rating, subjects, and Microsoft-Photo keywords. The description
  and rating fields are preserved on read-modify-write even though
  Magpie's UI no longer surfaces them.
- `common.rs` — shared `atomic_write_bytes`, EXIF → technical
  metadata, dimensions, and `write_not_supported_error` (for
  read-only stubs).
- `jpeg.rs`, `png.rs`, `webp.rs`, `gif.rs` — writable handlers.
  Each parses the format's container, drops any old XMP block,
  and splices in a freshly-built packet.
- `tiff.rs`, `stubs.rs` — read-only handlers for TIFF, HEIC, PDF,
  video, camera RAW, BMP, EXR, HDR, SVG, and more.

Windows Explorer's *Tags* column resolves from either `dc:subject`
or `MicrosoftPhoto:LastKeywordXMP`, so Magpie only emits the
standard Dublin Core form.

## `core/metadata/write.rs`

`write_metadata_to_source` is the single entry point for saves and
is now a thin façade over the registry:

1. **Handler lookup.** `registry.for_ext(ext).ok_or(UnsupportedFormat)?`.
2. **Handler write.** `handler.write_user(path, &meta)?`. The
   handler:
   - reads current on-disk state,
   - merges the incoming edits with `xmp_packet::merge_user_edits`
     (preserving foreign fields),
   - rebuilds the packet,
   - splices it into the container,
   - writes atomically via `common::atomic_write_bytes`.
3. **Best-effort delete** of any leftover `<file>.xmp` from a
   previous Magpie version or from Lightroom.

Failure semantics:

- Unsupported format = `Err(_)` before any disk touch (read-only
  handler's `write_user` returns immediately).
- Embed failure (read-only file, disk full, corrupt file) = `Err(_)`.
- Sidecar cleanup failure = logged as WARN, save still returns
  `Ok(())` because the source file already carries the metadata.

## `commands/*.rs`

Each file exposes 3–5 `#[tauri::command]` async functions. See
[Tauri command reference](./commands.md) for the full list.

Common patterns:

- `services: State<'_, Arc<AppServices>>` for shared state.
- `app_handle: AppHandle` for emitting events.
- Return `AppResult<T>`; Serde does the JSON legwork.
- Tracing spans on entry so `app.log` shows every IPC hit.
