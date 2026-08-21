# Tauri command reference

Every command below is registered in `src-tauri/src/lib.rs` inside
`tauri::generate_handler![…]`. All are `async fn`, return
`AppResult<T>`, and use camelCase JSON on the wire.

## Library management

### `add_library_folder`

```rust
async fn add_library_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    path: String,
) -> AppResult<LibraryFolder>;
```

Add a folder to the library and start a background scan. Emits
`app://scan` progress events. Returns the created row.

### `remove_library_folder`

```rust
async fn remove_library_folder(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()>;
```

Removes the folder row (CASCADE removes every image and image_tags
row). Files on disk are untouched.

### `list_library_folders`

```rust
async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<LibraryFolder>>;
```

### `rescan_folder`

```rust
async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> AppResult<ScanResult>;
```

Re-walks the specified folder. Incremental: only files with a
newer `mtime` than what's in the DB get re-processed.

### `rescan_all`

```rust
async fn rescan_all(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
) -> AppResult<Vec<ScanResult>>;
```

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

Paginated grid query. `filter` composes:

- `folder_id: Option<i64>`
- `tag: Option<String>`
- `search: Option<String>` — passed through FTS5.

Returns `Page { items, total, page, page_size }`.

### `get_image`

```rust
async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<ImageDetails>;
```

Returns full details for one image. **Side effect:** if the source
file's mtime (or any legacy `.xmp` sidecar's mtime) is newer than
`meta_read_at`, re-reads metadata from disk and updates the DB
*before* returning.

### `update_image_metadata`

```rust
async fn update_image_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    patch: MetadataPatch,
) -> AppResult<ImageDetails>;
```

Apply a metadata patch, embed the merged XMP into the source file,
delete any legacy `.xmp` sidecar, and return the new state. Emits
`app://image-updated`. Returns `Err` on unsupported formats
(anything other than JPEG or PNG) or file-write failures.

### `batch_update_metadata`

```rust
async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> AppResult<Vec<i64>>;
```

Same as `update_image_metadata` but for many photos. Returns the
list of IDs that succeeded; failures are logged and skipped.

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
any legacy `.xmp` sidecar + thumbnails + DB row. Returns per-file
success/failure.

## Tags

### `list_tags`

```rust
async fn list_tags(
    services: State<'_, Arc<AppServices>>,
    prefix: Option<String>,
) -> AppResult<Vec<TagStats>>;
```

Every tag with a photo count. Optional prefix filter for
autocompletion.

### `rename_tag`

```rust
async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    old_name: String,
    new_name: String,
) -> AppResult<()>;
```

Global rename across every photo. Rewrites every affected embedded
XMP packet in the source files.

### `delete_tag`

```rust
async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    name: String,
) -> AppResult<()>;
```

Remove a tag from every photo. Files are updated the same way as
rename.

## Smart collections (skeleton)

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

## Thumbnails and image paths

### `get_thumb_path`

```rust
async fn get_thumb_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
    size: ThumbSize,
) -> AppResult<String>;
```

Returns the absolute path of a cached thumbnail; generates it on
demand if missing.

### `get_image_path`

```rust
async fn get_image_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<String>;
```

Returns the absolute path of the source image (used by the details
panel to render a large preview via `convertFileSrc`).

## Diagnostics

### `log_frontend`

```rust
fn log_frontend(level: String, msg: String) -> AppResult<()>;
```

Push a log line into `app.log` from the renderer. `level` is one
of `debug|info|warn|error`. Emitted with `target="frontend"` for
easy filtering.
