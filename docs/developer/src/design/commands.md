# Tauri command reference

Every command below is registered in `src-tauri/src/lib.rs` inside
`tauri::generate_handler![…]`. All are `async fn`, return
`PicOrgResult<T>`, and use camelCase JSON on the wire.

## Library management

### `add_library_folder`

```rust
async fn add_library_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    path: String,
) -> PicOrgResult<LibraryFolder>;
```

Add a folder to the library and start a background scan. Emits
`picorg://scan` progress events. Returns the created row.

### `remove_library_folder`

```rust
async fn remove_library_folder(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<()>;
```

Removes the folder row (CASCADE removes every image and image_tags
row). Files on disk are untouched.

### `list_library_folders`

```rust
async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> PicOrgResult<Vec<LibraryFolder>>;
```

### `rescan_folder`

```rust
async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> PicOrgResult<ScanResult>;
```

Re-walks the specified folder. Incremental: only files with a
newer `mtime` than what's in the DB get re-processed.

### `rescan_all`

```rust
async fn rescan_all(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
) -> PicOrgResult<Vec<ScanResult>>;
```

## Images

### `query_images`

```rust
async fn query_images(
    services: State<'_, Arc<AppServices>>,
    filter: Option<ImageFilter>,
    sort: Option<ImageSort>,
    page: Option<Pagination>,
) -> PicOrgResult<Page<ImageSummary>>;
```

Paginated grid query. `filter` composes:

- `folder_id: Option<i64>`
- `min_rating: Option<i64>`
- `tag: Option<String>`
- `search: Option<String>` — passed through FTS5.

Returns `Page { items, total, page, page_size }`.

### `get_image`

```rust
async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<ImageDetails>;
```

Returns full details for one image. **Side effect:** if the source
file's / sidecar's mtime is newer than `meta_read_at`, re-reads
metadata from disk and updates the DB *before* returning.

### `update_image_metadata`

```rust
async fn update_image_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    patch: MetadataPatch,
) -> PicOrgResult<ImageDetails>;
```

Apply a metadata patch, write sidecar + embedded XMP, return the
new state. Emits `picorg://image-updated`.

### `batch_update_metadata`

```rust
async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> PicOrgResult<Vec<i64>>;
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
) -> PicOrgResult<DeleteResult>;
```

Move to Recycle Bin (default) or permanently delete. Also removes
sidecar + thumbnails + DB row. Returns per-file success/failure.

## Tags

### `list_tags`

```rust
async fn list_tags(
    services: State<'_, Arc<AppServices>>,
    prefix: Option<String>,
) -> PicOrgResult<Vec<TagStats>>;
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
) -> PicOrgResult<()>;
```

Global rename across every photo. Rewrites every affected sidecar +
embedded XMP.

### `delete_tag`

```rust
async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    name: String,
) -> PicOrgResult<()>;
```

Remove a tag from every photo. Files are updated the same way as
rename.

## Smart collections (skeleton)

### `list_smart_collections`

```rust
async fn list_smart_collections(
    services: State<'_, Arc<AppServices>>,
) -> PicOrgResult<Vec<SmartCollection>>;
```

### `create_smart_collection`

```rust
async fn create_smart_collection(
    services: State<'_, Arc<AppServices>>,
    name: String,
    filter: ImageFilter,
) -> PicOrgResult<SmartCollection>;
```

### `delete_smart_collection`

```rust
async fn delete_smart_collection(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<()>;
```

## Thumbnails and image paths

### `get_thumb_path`

```rust
async fn get_thumb_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
    size: ThumbSize,
) -> PicOrgResult<String>;
```

Returns the absolute path of a cached thumbnail; generates it on
demand if missing.

### `get_image_path`

```rust
async fn get_image_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<String>;
```

Returns the absolute path of the source image (used by the details
panel to render a large preview via `convertFileSrc`).

## Diagnostics

### `log_frontend`

```rust
fn log_frontend(level: String, msg: String) -> PicOrgResult<()>;
```

Push a log line into `picorg.log` from the renderer. `level` is one
of `debug|info|warn|error`. Emitted with `target="frontend"` for
easy filtering.
