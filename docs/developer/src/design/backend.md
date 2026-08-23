# Backend modules

## `lib.rs` — app entry

Responsibilities:

- Initialise file-based logging (`init_logging`).
- Build `AppServices { pool, thumb_cache_dir, app_data_dir, formats }`
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
  image row. `id` is a packed global ID.
- `MetadataPatch` — the "what to change" payload for
  `update_image_metadata` and `batch_update_metadata`.
- `SyncRiskWarning` — non-null result of `check_folder_sync_risk`.
- `double_option` module — custom Serde deserializer for
  `Option<Option<T>>`. Distinguishes "field missing" from
  "field explicitly null" on the wire.

Every struct uses `#[serde(rename_all = "camelCase")]` so JSON keys
are camelCase but Rust fields stay snake_case.

## `error.rs`

`AppError` enum (via `thiserror`):

- `Io(std::io::Error)` — filesystem errors.
- `Db(rusqlite::Error)` — DB errors.
- `Pool(String)` — LibraryPool / mutex-poisoning errors.
- `MetadataRead(String)` — user-facing metadata read failures.
- `Scan(String)` — scanner-specific issues.
- `Internal(String)` — anything else.

All commands return `AppResult<T>` (= `Result<T, AppError>`).
The `Display` impl for `AppError` is safe to surface to the user
(no absolute paths in strings without context).

## `db/` — two-tier storage

See [Database schema](./schema.md) and
[Database redesign](./db-redesign.md).

- `db/mod.rs` — packed global ID helpers.
- `db/registry.rs` — central `registry.db` schema (`library_folders`,
  `smart_collections`, `app_settings`) plus insert/list/update/delete
  helpers.
- `db/library.rs` — per-folder `library.db` schema (`folder_meta`,
  `images`, `tags`, `image_tags`, `images_fts`) plus
  `upsert_image`, `set_image_meta`, `apply_metadata_patch`,
  `rename_tag`, `delete_tag`, and FTS-rebuild helpers. Uses
  `LibraryDb::lock() -> MutexGuard<'_, Connection>` for writes.
- `db/pool.rs` — `LibraryPool`. Owns the registry connection with
  every library `ATTACH DATABASE`-ed on it, plus a lazy map of
  per-folder writer connections. `LibraryPool::with_registry`,
  `LibraryPool::library(folder_id)`, `LibraryPool::add_folder`,
  `LibraryPool::remove_folder`.
- `db/search.rs` — cross-folder query builders (`query_images`,
  `list_all_tags`, `get_image_by_gid`,
  `apply_metadata_patch_by_gid`, `group_gids_by_folder`).
- `db/legacy_migration.rs` — one-shot importer for the old central
  `library.db`.

## `core/scanner.rs`

Pipeline for `add_library_folder` and `rescan_*`:

1. **Walk.** `jwalk::WalkDir` traverses the folder in parallel.
2. **Filter.** Extensions checked against `IMAGE_EXTS`.
   `.magpie/library.db` (and WAL/SHM) is excluded from the walk.
3. **Diff.** For each file, look up by *folder-relative* path in
   `images`; skip if `mtime_ms` matches.
4. **Extract.** In parallel via `rayon`: EXIF read, XMP read (native
   handlers), Windows Shell property read (formats without native
   parsers), content hash (`XXH3`).
5. **Upsert.** DB writes are serialised via the folder's single
   `LibraryDb` mutex; scanner buffers batches to amortise txn
   overhead.
6. **Thumbnails.** Enqueue small + medium keyed by packed global ID.
7. **Progress.** Emit `app://scan { folder_id, done, total, current }`.

Scanning is bounded by disk read speed on cold cache and by CPU on
warm cache.

## `core/thumbnail.rs`

```rust
pub fn ensure_thumbnails(cache_dir: &Path, src: &Path, gid: i64) -> Result<()>;
pub fn thumb_path(cache_dir: &Path, gid: i64, size: ThumbSize) -> PathBuf;
pub fn delete_thumbnails(cache_dir: &Path, gid: i64);
```

Decode via `image::open`, resize via `fast_image_resize` (SIMD
Lanczos3), encode via `webp::Encoder` at quality 80. `gid` is the
packed global ID (`folder_id * 1_000_000_000 + local_id`), so
thumbnails for different folders never collide.

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
implementation, and every handler is now **read-only** (see
[Database redesign](./db-redesign.md)):

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
`db::library::apply_metadata_patch` and lives in the folder's
`library.db`.

## `commands/*.rs`

Each file exposes 3–5 `#[tauri::command]` async functions. See
[Tauri command reference](./commands.md) for the full list.

Common patterns:

- `services: State<'_, Arc<AppServices>>` for shared state.
- `app_handle: AppHandle` for emitting events.
- Return `AppResult<T>`; Serde does the JSON legwork.
- Tracing spans on entry so `app.log` shows every IPC hit.

Global IDs are unpacked at the command boundary — commands call
`db::unpack_global_id(id)` (or the higher-level
`search::get_image_by_gid` / `search::apply_metadata_patch_by_gid`)
so per-folder logic never sees the packed form.
