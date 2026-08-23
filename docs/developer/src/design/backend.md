# Backend modules

## `lib.rs` — app entry

Responsibilities:

- Initialise file-based logging (`init_logging`).
- Build `AppServices { db, thumb_cache_dir, app_data_dir, formats }`
  under `Arc`.
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

- `LibraryFolder` — one registered folder; includes `isAvailable`.
- `ImageSummary`, `ImageDetails` — full and abridged views of an
  image row. `id` is the plain `images.id` primary key.
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
- `Pool(String)` — mutex-poisoning errors on the shared `Db`
  handle. (Legacy variant name; kept for compatibility.)
- `MetadataRead(String)` — user-facing metadata read failures.
- `Internal(String)` — anything else.

All commands return `AppResult<T>` (= `Result<T, AppError>`).
The `Display` impl for `AppError` is safe to surface to the user
(no absolute paths in strings without context).

## `db/` — single central storage

See [Database schema](./schema.md) and
[Database design](./db-redesign.md).

- `db/mod.rs` — the `Db` handle wrapping
  `Arc<Mutex<Connection>>` plus `open`, `with_conn`,
  `with_conn_mut`. Defines `DB_FILE_NAME = "magpie.db"` and
  `SCHEMA_VERSION`.
- `db/schema.rs` + `db/schema.sql` — DDL for a fresh DB and the
  version check for an existing one.
- `db/queries.rs` — every query in one file, grouped by concern:
  folders, images (upsert / meta / delete), MetadataPatch,
  tag rename/delete, search (`query_images`, `list_all_tags`),
  smart collections.
- `db/migrate.rs` — startup importer. `open_or_migrate(app_data_dir)`
  detects a legacy `registry.db` or pre-redesign `library.db` and
  copies the data into `magpie.db` in one shot.

## `core/scanner.rs`

Pipeline for `add_library_folder` and `rescan_*`:

1. **Walk.** `jwalk::WalkDir` traverses the folder in parallel.
2. **Filter.** Extensions checked against
   `FormatRegistry::all_extensions`. Leftover `.magpie/library.db`
   (from a failed prior migration) is excluded from the walk.
3. **Diff.** For each file, look up by `(folder_id, rel_path)` in
   `images`; skip if `mtime_ms` matches.
4. **Extract.** In parallel via a `tokio` semaphore: EXIF read, XMP
   read (native handlers), Windows Shell property read (formats
   without native parsers), content hash (`XXH3`).
5. **Upsert.** DB writes are serialised via the `Db` mutex; each
   write is a short single-statement borrow.
6. **Thumbnails.** Enqueue small + medium keyed by `images.id`.
7. **Progress.** Emit `app://scan { folder_id, processed, total, current }`.

Scanning is bounded by disk read speed on cold cache and by CPU on
warm cache.

## `core/thumbnail.rs`

```rust
pub fn ensure_thumbnails(cache_dir: &Path, src: &Path, image_id: i64) -> Result<()>;
pub fn thumb_path(cache_dir: &Path, image_id: i64, size: ThumbSize) -> PathBuf;
pub fn delete_thumbnails(cache_dir: &Path, image_id: i64);
```

Decode via `image::open`, resize via `fast_image_resize` (SIMD
Lanczos3), encode via `webp::Encoder` at quality 80. `image_id` is
the plain `images.id` primary key — globally unique because there's
only one central DB.

## `core/metadata/read.rs`

`read_all(registry, path) -> ImageMetaFromFile` runs the read
pipeline:

1. Handler's `read_technical` for dimensions + EXIF-derived fields.
2. Handler's `read_user` for XMP-parseable formats.
3. Windows Shell property store fallback for RAW / HEIC / MP4 / PDF.
4. Legacy `.xmp` sidecar read for backward compatibility (no
   writes).

Non-fatal errors are collected into a per-file warning log; the
scanner continues on the next file.

The full pipeline is described in
[Metadata read path](./metadata-read.md).

## `core/formats/`

Each supported extension is owned by one `FormatHandler`
implementation, and every handler is **read-only** (see
[Database design](./db-redesign.md)):

- `mod.rs` — declares the `FormatHandler` trait
  (`name`, `extensions`, `kind`, `read_technical`, `read_user`),
  `TechnicalMeta`, `UserMeta`, `FormatKind`, and the
  `FormatRegistry` that lives on `AppServices`.
- `xmp_packet.rs` — hand-written streaming XMP parser (no writer).
  Extracts packets containing title, subjects, description,
  and Microsoft-Photo keywords.
- `common.rs` — shared helpers: EXIF → technical metadata,
  dimensions, verbatim-prefix stripping.
- `jpeg.rs`, `png.rs`, `webp.rs`, `gif.rs`, `tiff.rs` — native
  readers.
- `stubs.rs` — HEIF, video, PDF, RAW, BMP/EXR/HDR/SVG readers.
  These lean on `imagesize` for dimensions and defer user metadata
  to `win_shell::read_user_meta`.
- `win_shell.rs` — Windows-only Shell property store **reader**.

## No `metadata/write.rs`

The write path is gone. `FormatHandler` has no `write_user`;
`common::atomic_write_bytes`, `win_shell::write_user_meta`,
`xmp_packet::build_xmp_packet`, and `xmp_packet::merge_user_edits`
were all removed. Every user-metadata mutation now goes through
`db::queries::apply_metadata_patch` and lives in `magpie.db`.

## `commands/*.rs`

Each file exposes 3–5 `#[tauri::command]` async functions. See
[Tauri command reference](./commands.md) for the full list.

Common patterns:

- `services: State<'_, Arc<AppServices>>` for shared state.
- `services.db.with_conn(...)` / `with_conn_mut(...)` for every SQL
  operation.
- `app_handle: AppHandle` for emitting events.
- Return `AppResult<T>`; Serde does the JSON legwork.
- Tracing spans on entry so `app.log` shows every IPC hit.
