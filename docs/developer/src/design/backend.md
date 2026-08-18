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

`PicOrgError` enum (via `thiserror`):

- `Io(std::io::Error)` — filesystem errors.
- `Db(rusqlite::Error)` — DB errors.
- `MetadataRead(String)`, `MetadataWrite(String)` — user-facing
  metadata failures.
- `Scan(String)` — scanner-specific issues.
- `Internal(String)` — anything else.

All commands return `PicOrgResult<T>` (= `Result<T, PicOrgError>`).
The `Display` impl for `PicOrgError` is safe to surface to the user
(no absolute paths in strings without context).

## `db/mod.rs`

Thin wrapper around `rusqlite::Connection`:

```rust
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> PicOrgResult<Self> { … }

    pub fn with_conn<F, T>(&self, f: F) -> PicOrgResult<T>
    where
        F: FnOnce(&Connection) -> PicOrgResult<T>,
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
7. **Progress.** Emit `picorg://scan { done, total, current }`.

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
2. `xmp::extract_embedded_xmp(path)` for embedded XMP.
3. `read_sidecar(sidecar_path_for(path))` for sidecar XMP.
4. Merge sidecar-over-embedded via `apply_user_meta`.

Non-fatal errors are collected into a per-file warning log; the
scanner continues on the next file.

## `core/metadata/xmp.rs`

Everything XMP:

- `extract_embedded_xmp(path)` — JPEG APP1 walker (used to be the
  only reader path).
- `parse_user_metadata(bytes)` — streaming `quick_xml` parser that
  extracts the title / description / rating / subjects / MSFT
  keywords we care about.
- `build_xmp_packet(&UserMetadata)` — writes a standard XMP packet
  with both `dc:subject` and `MicrosoftPhoto:LastKeywordXMP` for
  round-trip with Windows Explorer.
- `embed_xmp_in_source(path, packet_bytes)` — JPEG-only writer that
  replaces or inserts an APP1 XMP segment atomically.

## `core/metadata/write.rs`

`merge_and_write_sidecar` is the single entry point for saves:

1. Read the *current* on-disk metadata (so we don't drop fields we
   don't touch).
2. Apply the patch fields (title / description / rating / subjects).
3. `write_sidecar` — atomic temp+rename to `Photo.xmp`.
4. `embed_xmp_in_source` — atomic temp+rename inside the source
   file if it's a JPEG.

Failure semantics:

- Sidecar failure = whole operation fails (caller sees `Err`).
- Embed failure = logged, sidecar is still written, whole op returns
  `Ok`. Rationale: the sidecar is the authoritative fallback, and
  losing embed on a read-only network share shouldn't kill an edit.

## `commands/*.rs`

Each file exposes 3–5 `#[tauri::command]` async functions. See
[Tauri command reference](./commands.md) for the full list.

Common patterns:

- `services: State<'_, Arc<AppServices>>` for shared state.
- `app_handle: AppHandle` for emitting events.
- Return `PicOrgResult<T>`; Serde does the JSON legwork.
- Tracing spans on entry so `picorg.log` shows every IPC hit.
