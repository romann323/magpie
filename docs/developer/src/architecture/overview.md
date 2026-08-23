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
     %APPDATA%\com.magpie.app\registry.db              Filesystem
       └── ATTACH DATABASE                             Your photos
       ↳ f1: <folder1>\.magpie\library.db             (never modified;
       ↳ f2: <folder2>\.magpie\library.db              read-only source)
       ↳ ...
```

Two big ideas:

1. **The renderer is a dumb view.** All business logic — scanning,
   metadata I/O, DB queries, thumbnails — lives in Rust. The
   renderer's job is to invoke commands and render their results.
2. **The database is the source of truth for user metadata.** Each
   registered folder gets its own portable `library.db` inside a
   hidden `.magpie` subfolder. The central `registry.db` in
   `%APPDATA%` only knows which folders are registered. Source
   files are read-only — Magpie doesn't modify their bytes.

See [Database redesign](../design/db-redesign.md) for the deeper
walkthrough.

## Sub-systems

### App shell (Tauri)

Owns the window, capability model, WebView2 host, and the IPC bridge.
The `AppServices` struct is constructed once in `lib.rs` and injected
into every Tauri command via `State<Arc<AppServices>>`. It contains:

- `pool: Arc<LibraryPool>` — owns the registry connection (with
  every library `ATTACH DATABASE`-ed on it) and lazy per-folder
  writer connections.
- `thumb_cache_dir: PathBuf` — location of the thumbnail cache.
- `formats: Arc<FormatRegistry>` — registry of read-only format
  handlers.

### DB layer (`src-tauri/src/db/`)

- `db/registry.rs` — schema and query functions for the central
  `registry.db`. Tables: `library_folders`, `smart_collections`,
  `app_settings`.
- `db/library.rs` — schema and query functions for per-folder
  `library.db`. Tables: `folder_meta`, `images`, `tags`,
  `image_tags`, `images_fts`.
- `db/pool.rs` — `LibraryPool` type. Attaches every registered
  library to the registry connection on startup so cross-folder
  queries run against a single connection.
- `db/search.rs` — cross-folder query builders (`query_images`,
  `list_all_tags`) using `UNION ALL` over attached schemas.
- `db/legacy_migration.rs` — one-shot importer for the old central
  `library.db` (pre-redesign).
- `db/mod.rs` — packed global ID helpers
  (`pack_global_id`, `unpack_global_id`).

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
5. Upsert into the folder's `library.db` (paths stored
   folder-relative).
6. Enqueue thumbnail generation keyed by the packed global ID.

Progress events flow to the frontend via `app://scan`.

### Thumbnail pipeline (`src-tauri/src/core/thumbnail.rs`)

Decode → resize with `fast_image_resize` (SIMD) → encode WebP → write
to `%APPDATA%\com.magpie.app\thumbs\<gid>-<size>.webp`. Two sizes
per photo (small ~256 px, medium ~512 px). `<gid>` is
`pack_global_id(folder_id, local_id)`.

### Metadata (`src-tauri/src/core/metadata/` + `src-tauri/src/core/formats/`)

Post-redesign this is a **read-only** pipeline:

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
  **reader** (also read-only after the redesign).

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
5. Rust's `get_image` unpacks the global ID into `(folder_id,
   local_id)`, looks up the folder root in the registry, and reads
   the image row from the folder's `library.db`. If the source
   file's `mtime` moved forward since import, `read_all` re-reads
   metadata and updates the row.
6. Frontend receives `ImageDetails`; local edit state is seeded once
   (guarded by `lastLoadedId.current === q.data.id`).
7. User types in the Title input. `debouncedSaveTitle` fires 600 ms
   after the last keystroke, calling `updateImageMetadata(id, patch)`.
8. Rust's `update_image_metadata` unpacks the ID, opens the folder's
   library, applies the patch inside one transaction, rebuilds the
   FTS row, and emits `app://image-updated`. **The source file is
   not touched.**
9. Frontend's `qc.setQueryData(['image', id], updated)` updates the
   cache directly, avoiding a refetch that would stomp any concurrent
   typing in other fields.
