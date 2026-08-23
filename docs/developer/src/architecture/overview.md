# High-level architecture

## The 30-second version

```
   ┌───────────────────────────────────────────────────────────┐
   │                    Magpie process                         │
   │                                                           │
   │   ┌──────────────────┐         ┌─────────────────────┐    │
   │   │  Renderer        │  IPC    │  Rust core           │   │
   │   │  (WebView2)      │◀───────▶│  (Tokio + threads)   │   │
   │   │  React + TS      │         │                     │    │
   │   └──────────────────┘         └──┬────────┬─────────┘    │
   │                                   │        │              │
   └───────────────────────────────────┼────────┼──────────────┘
                                       │        │
                       ┌───────────────┘        └────────────────┐
                       ▼                                         ▼
     %APPDATA%\com.magpie.app\magpie.db                Filesystem
       ├── library_folders                              Your photos
       ├── images                                       (never modified;
       ├── tags + image_tags                             read-only source)
       ├── images_fts (FTS5)
       └── smart_collections, app_settings, schema_meta
```

Two big ideas:

1. **The renderer is a dumb view.** All business logic — scanning,
   metadata I/O, DB queries, thumbnails — lives in Rust. The
   renderer's job is to invoke commands and render their results.
2. **The database is the source of truth for user metadata.**
   Everything lives in one SQLite file under `%APPDATA%`. Source
   files are read-only — Magpie doesn't modify their bytes.

See [Database design](../design/db-redesign.md) for the deeper
walkthrough.

## Sub-systems

### App shell (Tauri)

Owns the window, capability model, WebView2 host, and the IPC bridge.
The `AppServices` struct is constructed once in `lib.rs` and injected
into every Tauri command via `State<Arc<AppServices>>`. It contains:

- `db: Db` — thin `Send + Sync` handle around a single
  `rusqlite::Connection`.
- `thumb_cache_dir: PathBuf` — location of the thumbnail cache.
- `formats: Arc<FormatRegistry>` — registry of read-only format
  handlers.

### DB layer (`src-tauri/src/db/`)

- `db/mod.rs` — the `Db` handle (`Arc<Mutex<Connection>>`),
  `open`, `with_conn`, `with_conn_mut`.
- `db/schema.rs` + `db/schema.sql` — DDL applied on a fresh DB.
- `db/queries.rs` — every SQL statement grouped by concern
  (folders, images, tags, search, smart collections).
- `db/migrate.rs` — startup importer for legacy layouts.

See [Database schema](../design/schema.md).

### Scanner (`src-tauri/src/core/scanner.rs`)

Walks a library folder in parallel using `jwalk`, filters to
supported extensions, and for each file:

1. Stat + skip if `mtime_ms` unchanged.
2. Hash content (`XXH3`).
3. Extract EXIF (taken time, dimensions, camera).
4. Extract XMP from embedded chunks (JPEG APP1 / PNG iTXt / etc.)
   and, on Windows, from the Shell property store for formats we
   don't natively parse. Existing tags are imported on first sight.
5. Upsert into `magpie.db` (paths stored folder-relative under
   `library_folders.path`).
6. Enqueue thumbnail generation keyed by `images.id`.

Progress events flow to the frontend via `app://scan`.

### Thumbnail pipeline (`src-tauri/src/core/thumbnail.rs`)

Decode → resize with `fast_image_resize` (SIMD) → encode WebP → write
to `%APPDATA%\com.magpie.app\thumbs\<id>-<size>.webp`. Two sizes
per photo (small ~256 px, medium ~512 px). `<id>` is `images.id`.

### Metadata (`src-tauri/src/core/metadata/` + `src-tauri/src/core/formats/`)

A **read-only** pipeline:

- `metadata/read.rs` — asks the format registry for the right
  handler, calls `read_technical` + `read_user`, then falls back to
  the Windows Shell property store on Windows for formats we don't
  natively parse.
- `metadata/sidecar.rs` — reads legacy `.xmp` sidecar files for
  backward compatibility. No new sidecars are ever written.
- `formats/mod.rs` — declares the `FormatHandler` trait
  (`name`, `extensions`, `kind`, `read_technical`, `read_user`) and
  the `FormatRegistry` that owns one instance per supported
  extension.
- `formats/xmp_packet.rs` — read-only XMP parser
  supporting the subset Magpie cares about (title, subjects,
  MicrosoftPhoto keywords, description).
- `formats/{jpeg,png,webp,gif,tiff}.rs` — native readers.
- `formats/stubs.rs` — thin readers for HEIF, video, PDF, RAW, and
  basic raster formats that delegate technical reads to `imagesize`
  and defer user-meta reads to the Windows Shell path.
- `formats/win_shell.rs` — Windows-only Shell property store
  **reader**.

See [File formats](../design/file-formats.md) for the full
handler catalogue and
[Adding a format handler](../design/adding-a-format-handler.md) for
the plug-in recipe.

### Command surface (`src-tauri/src/commands/`)

About 20 async Tauri commands grouped by concern (`library`,
`images`, `tags`, `collections`, `thumbs`, `diag`). Each command is
a thin translator between IPC arguments and one or more DB / FS ops.
See [Tauri command reference](../design/commands.md).

### Frontend (`src/`)

- `App.tsx` — root, wires TopBar / Sidebar / ImageGrid / DetailsPanel.
- `features/` — one component per major UI area.
- `ipc.ts` — typed wrappers around `invoke(...)`.
- `store.ts` — Zustand state (selection, view, sort, filters).
- `types.ts` — Rust-mirrored TypeScript types.

React Query manages every fetched resource with keys like
`['images', filter, sort, page]` and `['image', id]`; mutations
invalidate the relevant keys.

## Data flow of a typical click

Selecting a photo and typing a title, end to end:

1. User clicks a tile in `ImageGrid`.
2. `useStore.setSelection([id])`.
3. `DetailsPanel` switches to `SingleDetails id={id}`.
4. `useQuery(['image', id])` fires; TanStack Query calls
   `getImage(id)` → invokes `get_image` command.
5. Rust's `get_image` looks up the image row plus its folder root
   from `magpie.db`. If the source file's `mtime` moved forward
   since import, `read_all` re-reads metadata and updates the row.
6. Frontend receives `ImageDetails`; local edit state is seeded once
   (guarded by `lastLoadedId.current === q.data.id`).
7. User types in the Title input. `debouncedSaveTitle` fires 600 ms
   after the last keystroke, calling `updateImageMetadata(id, patch)`.
8. Rust's `update_image_metadata` applies the patch inside one
   transaction, rebuilds the FTS row, and emits
   `app://image-updated`. **The source file is not touched.**
9. Frontend's `qc.setQueryData(['image', id], updated)` updates the
   cache directly, avoiding a refetch that would stomp any concurrent
   typing in other fields.
