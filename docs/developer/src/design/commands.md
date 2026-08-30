# Tauri command reference

Every command below is registered in `src-tauri/src/lib.rs` inside
`tauri::generate_handler![…]`. Signatures use `AppResult<T>` and
camelCase JSON on the wire.

> **IDs.** Every `id: i64` an image command accepts or returns is the
> plain `images.id` primary key of the currently-open project's DB.
>
> **`NoProjectOpen`.** Every command below (except the `project::*`
> commands and `settings::*` commands) fails with
> `AppError::NoProjectOpen` when the user hasn't opened or created a
> project yet.

## Project management

### `current_project`

```rust
async fn current_project(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Option<ProjectInfo>>;
```

Returns the currently-open project (`{ path, name }`) or `null` when
none is open. The frontend calls this on mount to decide between the
welcome screen and the main layout.

### `create_project`

```rust
async fn create_project(
    services: State<'_, Arc<AppServices>>,
    path: String,
) -> AppResult<ProjectInfo>;
```

Create a fresh SQLite DB at `path` and swap it in as the current
project. The `.magpie` extension is appended when `path` has none.
Fails with `BadInput` if `path` already exists.

### `open_project`

```rust
async fn open_project(
    services: State<'_, Arc<AppServices>>,
    path: String,
) -> AppResult<ProjectInfo>;
```

Open an existing project file and make it the current project. Fails
with `PathNotFound` if the file isn't there.

### `save_project`

```rust
async fn save_project(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<ProjectInfo>;
```

No-op (SQLite writes on every mutation). Kept so the menu item has a
handler and the file's current descriptor can be surfaced back to the
UI.

### `save_project_as`

```rust
async fn save_project_as(
    services: State<'_, Arc<AppServices>>,
    path: String,
) -> AppResult<ProjectInfo>;
```

Copy the current DB to `path` using SQLite's online backup API, then
reopen from the new location. Subsequent writes hit the new file.

### `close_project`

```rust
async fn close_project(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<()>;
```

Drop the current project handle. The frontend then shows the welcome
screen.

## App settings

### `get_app_settings`

```rust
async fn get_app_settings(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<AppSettings>;
```

Return the persisted `AppSettings` blob (theme, font size, language,
last-project path, recent projects list).

### `update_app_settings`

```rust
async fn update_app_settings(
    services: State<'_, Arc<AppServices>>,
    patch: AppSettingsPatch,
) -> AppResult<AppSettings>;
```

Merge a `{ theme?, fontSize?, language? }` patch and write the file
atomically. Returns the new full settings.

## Menu control

### `set_menu_item_enabled`

```rust
fn set_menu_item_enabled(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: String,
    enabled: bool,
) -> AppResult<()>;
```

Toggle a menu item's enabled state. The frontend uses this to grey
`edit_undo` / `edit_redo` when the corresponding stack is empty.

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

When **Settings → Auto-tag photos** is enabled (`AppSettings.aiAutoTag`)
the scan is chained with an automatic-AI-tagging pass on the same
folder — see `core::auto_tag::tag_folder`. That pass emits
`app://auto-tag` progress events (see [Events](#events)) and never
runs on plain `rescan_folder` / `rescan_all`.

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

Return full details for one image. `ImageDetails.userTags` and
`ImageDetails.autoTags` come back separately so the DetailsPanel can
render editable vs. read-only pills.

**Side effect:** if the source file's `mtime` is newer than what's
stored in the row, the format handler is asked to re-read user
metadata (title + tags from XMP/`System.Keywords`) so that a
Lightroom / Explorer edit made after import is picked up on next
load. That re-read goes through `set_image_meta`, so any names the
file now reports become **additive `'auto'` tags** — user tags are
never overwritten. Fresh reads only update the row if the mtime
moved forward; there is no periodic polling.

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
`magpie.db`. **All tag fields target the `'user'` source** — auto
rows carried by the image are never inserted, removed, or renamed
by this command. `tags` replaces the current user-tag list; `tags_add`
inserts user rows for each name (no-op when already present as a
user tag; an auto row with the same name is unaffected); `tags_remove`
deletes user rows with matching names and leaves auto rows in place.

**The source file is never touched.** Rebuilds the row's FTS entry
so subsequent search reflects the change. Emits `app://image-updated`.

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

### `rename_image`

```rust
async fn rename_image(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    new_filename: String,
) -> AppResult<ImageDetails>;
```

Rename a single file on disk and update its DB row. Steps:

1. Validate `new_filename` (non-empty, no path separators, no illegal
   Windows filename characters).
2. Compute the new absolute path in the same parent directory as the
   current one.
3. Refuse if a file already exists at the target path.
4. `std::fs::rename` on disk.
5. `queries::rename_image_row` — updates `images.filename`,
   `images.rel_path`, and `images.ext`, then rebuilds the FTS row.
   Fails with `BadInput` if the new `rel_path` would collide with
   another row in the same folder.
6. On DB failure, roll the FS rename back (best-effort).

Emits `app://image-updated` on success.

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
`image_tags`. Aggregates both sources — an image with the same tag as
both `'auto'` and `'user'` is counted once via
`COUNT(DISTINCT it.image_id)`. Optional `prefix` narrows by
`LIKE ? COLLATE NOCASE`.

### `rename_tag`

```rust
async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    old_name: String,
    new_name: String,
) -> AppResult<()>;
```

Rename or merge. Operates on the shared `tags` vocabulary, so both
`'auto'` and `'user'` rows follow the rename automatically. If
`new_name` already exists, every `(image, source)` pair that pointed
at `old_name` gets re-pointed at `new_name` (preserving provenance);
the old vocabulary row is dropped. FTS rows for every affected image
are rebuilt in the same transaction. **Source files are not touched.**

### `delete_tag`

```rust
async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    name: String,
) -> AppResult<()>;
```

Remove a tag globally. `ON DELETE CASCADE` on the `image_tags` FK
cleans out both `'auto'` and `'user'` rows in one go. **Source files
are not touched.**

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

## Magnifier window

### `get_magnifier_context` / `set_magnifier_context` / `set_magnifier_current`

```rust
async fn get_magnifier_context(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<MagnifierContext>;

async fn set_magnifier_context(
    services: State<'_, Arc<AppServices>>,
    image_id: Option<i64>,
    filter: Option<ImageFilter>,
    sort: Option<ImageSort>,
) -> AppResult<()>;

async fn set_magnifier_current(
    services: State<'_, Arc<AppServices>>,
    image_id: Option<i64>,
) -> AppResult<()>;
```

The main window calls `set_magnifier_context` right before creating
the Magnifier `WebviewWindow`; the popup calls
`get_magnifier_context` on mount to know which image to display and
which filtered set to walk. As the user navigates with the arrow
keys, the popup calls `set_magnifier_current` so the "last shown"
pointer survives close/re-open.

None of these three commands touch the DB, so they work whether or
not a project is open.

## Events (Rust → JS)

| Event                | Payload                | Emitted by                              |
| -------------------- | ---------------------- | --------------------------------------- |
| `app://scan`         | `ScanProgress`         | `scanner::scan_folder` during scans.    |
| `app://auto-tag`     | `AutoTagProgress`      | `core::auto_tag::tag_folder` during an automatic-AI-tagging pass. |
| `app://image-updated`| `i64` (image id)       | Any command that mutates a row.         |
| `app://images-deleted`| `Vec<i64>`            | `delete_images` after a successful batch. |
| `app://menu`         | `String` (menu id)     | Every native menu click; routed by `App.tsx`. |

**`AutoTagProgress` payload:**

```typescript
type AutoTagProgress = {
  folderId: number
  processed: number
  total: number
  currentPath: string | null
  tagsAdded: number   // cumulative tags attached this run
  skipped: number     // images skipped because fingerprint still matches
  finished: boolean
}
```

On `finished: true` the frontend invalidates the `images`, `tags`,
and `image` query keys so the sidebar tag cloud and any open details
panel pick up the newly-attached tags.

### Menu commands

- `set_menu_item_enabled(id, enabled)` — used for context-sensitive
  items (`edit_undo`, `edit_redo`, `view_magnifier`).
- `set_menu_item_label(id, label)` — used to reflect stateful
  toggles that don't have a native check widget. Currently the only
  caller is `App.tsx` syncing the **Settings → Auto-tag photos**
  label (`… ✓` vs plain) to the persisted `aiAutoTag` setting.
