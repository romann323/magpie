# Tauri command reference

Every command below is registered in `src-tauri/src/lib.rs` inside
`tauri::generate_handler![…]`. Signatures use `AppResult<T>` and
camelCase JSON on the wire.

> **IDs.** Every `id: i64` an image command accepts or returns is the
> plain `images.id` primary key. It's globally unique because Magpie
> has a single central DB.

## Library management

### `add_library_folder`

```rust
async fn add_library_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    path: String,
) -> AppResult<LibraryFolder>;
```

Canonicalise the path, insert a row into `library_folders`, and
spawn a background scan. Emits `app://scan` progress events.

### `remove_library_folder`

```rust
async fn remove_library_folder(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()>;
```

Delete the `library_folders` row. Because `images.folder_id` has
`ON DELETE CASCADE`, all image rows, `image_tags`, and FTS rows for
that folder are removed in the same transaction. **Source files on
disk are untouched.**

### `list_library_folders`

```rust
async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<LibraryFolder>>;
```

Refreshes each folder's `isAvailable` flag by checking whether the
root can be stat-ed, then returns the list. Availability is now
purely about the filesystem; the DB itself is always present.

### `rescan_folder`

```rust
async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> AppResult<ScanResult>;
```

Re-walks the specified folder. Incremental: only files with a newer
`mtime` than what's in the DB get re-processed.

### `rescan_all`

```rust
async fn rescan_all(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
) -> AppResult<Vec<ScanResult>>;
```

Rescan every *available* folder sequentially.

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

Plain `SELECT ... FROM images WHERE ... ORDER BY ... LIMIT ...` —
one query against the single central DB. The result rows carry
absolute paths, built by joining `images.rel_path` onto the folder
root in Rust.

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

Apply a metadata patch (title, tags, `tags_add`, `tags_remove`) to
`magpie.db`. **The source file is never touched.** Rebuilds the
row's FTS entry so subsequent search reflects the change. Emits
`app://image-updated`.

### `batch_update_metadata`

```rust
async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> AppResult<Vec<i64>>;
```

Same as `update_image_metadata` but for many photos. Each `id` is
patched inside its own transaction; the command returns the list of
`ids` that succeeded (failures are logged and skipped).

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
the DB rows (via ON DELETE CASCADE) and cached thumbnails. Returns
per-file success/failure.

## Tags

### `list_tags`

```rust
async fn list_tags(
    services: State<'_, Arc<AppServices>>,
    prefix: Option<String>,
) -> AppResult<Vec<TagStats>>;
```

Straight `SELECT` from the central `tags` table joined against
`image_tags`. Optional `prefix` narrows by `LIKE ? COLLATE NOCASE`.

### `rename_tag`

```rust
async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    old_name: String,
    new_name: String,
) -> AppResult<()>;
```

Rename or merge. If `new_name` already exists, every image that had
`old_name` gets `new_name` added and the old row is dropped. FTS
rows for every affected image are rebuilt in the same transaction.
**Source files are not touched.**

### `delete_tag`

```rust
async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    name: String,
) -> AppResult<()>;
```

Remove a tag globally. **Source files are not touched.**

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

Smart collections live in the central `smart_collections` table
next to everything else.

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
if missing. Thumbnails are indexed by `images.id` and live under
`%APPDATA%\com.magpie.app\thumbs\`.

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
