# Tauri command reference

Every command below is registered in `src-tauri/src/lib.rs` inside
`tauri::generate_handler![…]`. Signatures use `AppResult<T>` and
camelCase JSON on the wire.

> **IDs are packed.** Every `id: i64` an image command accepts or
> returns is the *global* packed ID
> (`folder_id * 1_000_000_000 + local_id`). The frontend never sees
> per-folder local IDs directly.

## Library management

### `add_library_folder`

```rust
async fn add_library_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    path: String,
) -> AppResult<LibraryFolder>;
```

Canonicalises the path, inserts a row into `registry.db`, creates
the folder's `.magpie/library.db`, attaches it on the registry
connection, and spawns a background scan. Emits `app://scan`
progress events.

### `remove_library_folder`

```rust
async fn remove_library_folder(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()>;
```

Detach the library from the registry connection and delete the
`library_folders` row. **The `.magpie/library.db` file on disk is
left in place** — it's the user's data and Magpie doesn't own the
folder.

### `list_library_folders`

```rust
async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<LibraryFolder>>;
```

Includes `isAvailable` — `false` when the folder's library couldn't
be reached (removable drive unplugged, network share unreachable).

### `rescan_folder`

```rust
async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> AppResult<ScanResult>;
```

Re-walks the specified folder. Incremental: only files with a
newer `mtime` than what's in that folder's DB get re-processed.

### `rescan_all`

```rust
async fn rescan_all(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
) -> AppResult<Vec<ScanResult>>;
```

Rescan every *available* folder sequentially.

### `check_folder_sync_risk`

```rust
async fn check_folder_sync_risk(
    path: String,
) -> AppResult<Option<SyncRiskWarning>>;
```

Non-null when `path` looks like it lives on a cloud-synced disk
(OneDrive / Dropbox / Google Drive / iCloud / Box) or a UNC network
share. The frontend calls this **before** `add_library_folder` and
shows a `confirm` dialog with the returned message.

`SyncRiskWarning { provider: String, message: String }`.

## Images

### `query_images`

```rust
async fn query_images(
    services: State<'_, Arc<AppServices>>,
    filter: Option<ImageFilter>,
    sort: Option<ImageSort>,
    page: Option<Pagination>,
) -> AppResult<Page<ImageSummary>>;
```

Cross-folder query. Under the hood: `UNION ALL` across every
attached library DB, then order + paginate. Each result row's `id`
is packed with its `folder_id`.

### `get_image`

```rust
async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<ImageDetails>;
```

Return full details for one image. **Side effect:** if the source
file's `mtime` is newer than what's stored in the row, the format
handler is asked to re-read user metadata (title + tags from
XMP/`System.Keywords`) so that a Lightroom / Explorer edit made
after import is picked up on next load. Fresh reads only update the
row if the mtime moved forward; there is no periodic polling.

### `update_image_metadata`

```rust
async fn update_image_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    patch: MetadataPatch,
) -> AppResult<ImageDetails>;
```

Apply a metadata patch to the folder's `library.db` (title, tags,
`tags_add`, `tags_remove`). **The source file is never touched.**
Rebuilds the FTS row so subsequent search reflects the change.
Emits `app://image-updated`.

### `batch_update_metadata`

```rust
async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> AppResult<Vec<i64>>;
```

Same as `update_image_metadata` but for many photos. `ids` are
packed globals; the command groups them by folder and touches each
folder's `library.db` once inside a single transaction. Returns the
list of `ids` that succeeded; failures are logged and skipped.

### `delete_images`

```rust
async fn delete_images(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    permanent: Option<bool>,
) -> AppResult<DeleteResult>;
```

Move to Recycle Bin (default) or permanently delete. Also removes
the DB rows and cached thumbnails. Returns per-file success/failure.

## Tags

### `list_tags`

```rust
async fn list_tags(
    services: State<'_, Arc<AppServices>>,
    prefix: Option<String>,
) -> AppResult<Vec<TagStats>>;
```

Aggregates tags across every attached library via `UNION ALL`
grouped by `name COLLATE NOCASE`.

### `rename_tag`

```rust
async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    old_name: String,
    new_name: String,
) -> AppResult<()>;
```

Global rename across every available folder. Rebuilds FTS rows for
every affected image. **Source files are not touched.**

### `delete_tag`

```rust
async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    name: String,
) -> AppResult<()>;
```

Remove a tag from every folder's library. **Source files are not
touched.**

## Smart collections

### `list_smart_collections`

```rust
async fn list_smart_collections(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<SmartCollection>>;
```

### `create_smart_collection`

```rust
async fn create_smart_collection(
    services: State<'_, Arc<AppServices>>,
    name: String,
    filter: ImageFilter,
) -> AppResult<SmartCollection>;
```

### `delete_smart_collection`

```rust
async fn delete_smart_collection(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()>;
```

Smart collections live in `registry.db` (they're app-wide, not
per-folder).

## Thumbnails and image paths

### `get_thumb_path`

```rust
async fn get_thumb_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
    size: ThumbSize,
) -> AppResult<String>;
```

Return the absolute path of a cached thumbnail; generate on demand
if missing. Thumbnails are indexed by the *packed global ID* so
they don't collide between folders.

### `get_image_path`

```rust
async fn get_image_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<String>;
```

Absolute path of the source image (folder root + `rel_path`), for
the DetailsPanel inline preview via `convertFileSrc`.

## Diagnostics

### `log_frontend`

```rust
fn log_frontend(level: String, msg: String) -> AppResult<()>;
```

Push a log line into `app.log` from the renderer. `level` is one
of `debug|info|warn|error`. Emitted with `target="frontend"` for
easy filtering.
