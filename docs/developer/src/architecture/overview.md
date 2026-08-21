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
                ┌─────────────┐                           ┌─────────────┐
                │  SQLite DB  │                           │  Filesystem │
                │ library.db   │                           │ Your photos │
                └─────────────┘                           └─────────────┘
                (index only)                       (source of truth for
                                                    photos + XMP)
```

Two big ideas:

1. **The renderer is a dumb view.** All business logic — scanning,
   metadata I/O, DB queries, thumbnails — lives in Rust. The
   renderer's job is to invoke commands and render their results.
2. **The filesystem is the source of truth for user data.** The
   SQLite database is a cache/index; wiping it never loses user
   data — for the writable formats (JPEG/PNG/WebP/GIF) tags and
   titles live in the embedded XMP inside each source file; for the
   read-only formats they live in the DB and would need to be
   re-entered after a wipe.

## Sub-systems

### App shell (Tauri)

Owns the window, capability model, WebView2 host, and the IPC bridge.
The `AppServices` struct is constructed once in `lib.rs` and injected
into every Tauri command via `State<Arc<AppServices>>`. It contains:

- `db: Db` — a thread-safe SQLite handle wrapper.
- `cache_dir: PathBuf` — location of the thumbnail cache.
- (Future: settings, background scheduler handle.)

### DB layer (`src-tauri/src/db/`)

`Db` wraps `rusqlite::Connection` behind a `Mutex` and exposes a
`with_conn<F>` helper that hands out `&Connection` under lock. It's a
deliberate choice: SQLite is single-writer, and a plain mutex is
faster than a connection pool for our access pattern.

Migrations run at startup. See
[Database schema](../design/schema.md).

### Scanner (`src-tauri/src/core/scanner.rs`)

Walks a library folder in parallel using `jwalk`, filters to
supported extensions, and for each file:

1. Stat + skip if `mtime_ms` unchanged.
2. Hash content (`XXH3`).
3. Extract EXIF (taken time, dimensions, camera).
4. Extract XMP from embedded chunks (JPEG APP1 / PNG iTXt) and, if
   present, any legacy `.xmp` sidecar for backward compatibility.
5. Upsert into `images` + `image_tags`.
6. Enqueue thumbnail generation.

Progress events flow to the frontend via `app://scan`.

### Thumbnail pipeline (`src-tauri/src/core/thumbnail.rs`)

Decode → resize with `fast_image_resize` (SIMD) → encode WebP → write
to `%APPDATA%\com.magpie.app\thumbs\<id>-<size>.webp`. Two sizes
per photo (small ~256 px, medium ~512 px).

### Metadata (`src-tauri/src/core/metadata/` + `src-tauri/src/core/formats/`)

- `metadata/read.rs` — a thin façade that asks the format registry
  for the right handler and then folds in any legacy sidecar so
  users migrating from Lightroom / old Magpie don't lose data.
- `metadata/write.rs` — asks the registry for the handler, calls
  its atomic writer, and cleans up any legacy `.xmp`.
- `metadata/sidecar.rs` — computes the legacy sidecar path (read
  + cleanup only; Magpie never writes new sidecars).
- `formats/mod.rs` — declares the `FormatHandler` trait and the
  `FormatRegistry` that owns one instance per supported extension.
- `formats/xmp_packet.rs` — a hand-written streaming XMP
  reader/writer supporting the subset Magpie cares about (title,
  description, rating, subjects, MicrosoftPhoto keywords). The
  description and rating fields are preserved on read-modify-write
  even though Magpie's UI doesn't surface them.
- `formats/{jpeg,png,webp,gif}.rs` — writable handlers for the
  four image formats that have a safe embed path today.
- `formats/tiff.rs` and `formats/stubs.rs` — read-only handlers
  for TIFF, HEIC, PDF, video, and camera RAW.

See [File formats](../design/file-formats.md) for the full
handler catalogue and
[Adding a format handler](../design/adding-a-format-handler.md) for
the plug-in recipe.

### Command surface (`src-tauri/src/commands/`)

Roughly 25 async Tauri commands grouped by concern (`library`,
`images`, `tags`, `collections`, `thumbs`, `diag`). Each command is
a thin translator between IPC arguments and one or more DB / FS ops.

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
   `getImage(id)` (from `ipc.ts`) → invokes `get_image` command.
5. Rust's `get_image` checks whether the FS is newer than
   `meta_read_at`; if so, re-reads via `read_all` and
   `resync_user_meta_from_fs`.
6. Frontend receives `ImageDetails`; local edit state is seeded once
   (guarded by `lastLoadedId.current === q.data.id`).
7. User types in the Title input. `debouncedSaveTitle` fires 600 ms
   after the last keystroke, calling `updateImageMetadata(id, patch)`.
8. Rust's `update_image_metadata` runs `apply_metadata_patch`
   (transactional), then `write_metadata_to_source` (blocking,
   awaited — embeds the merged XMP into the source file and cleans
   up any legacy sidecar), then `set_meta_written_at` +
   `set_meta_read_at_now`, and finally emits
   `app://image-updated`.
9. Frontend's `qc.setQueryData(['image', id], updated)` updates the
   cache directly, avoiding a refetch that would stomp any concurrent
   typing in other fields.
