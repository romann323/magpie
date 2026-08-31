# Backend modules

## `lib.rs` — app entry

Responsibilities:

- Initialise file-based logging (`init_logging`).
- Build `AppServices { project, settings, thumb_cache_dir, app_data_dir, formats }`
  under `Arc`.
- Register every Tauri plugin (`dialog`, `opener`) and command
  handler in `invoke_handler!`.
- Build the native menu via `menu::build_menu` and forward every
  click to the frontend as an `app://menu` event.
- Manage the app run loop.

Notable pitfalls:

- `init_logging` **must not** call `tracing_subscriber::fmt().init()`
  if a Tauri plugin (like the removed `tauri-plugin-log`) already
  set a global subscriber. The prior implementation crashed with
  `PluginInitialization("log", "attempted to set a logger after…")`;
  the current version owns the logger outright.

## `core/mod.rs` — `AppServices`

`AppServices` is the shared state every command touches:

```rust
pub struct AppServices {
    project: Mutex<ProjectState>,
    settings: Mutex<AppSettings>,
    magnifier: Mutex<MagnifierContext>,
    pub app_data_dir: PathBuf,
    thumbs_root: PathBuf,
    pub formats: Arc<FormatRegistry>,
}
```

Key methods:

- `db() -> AppResult<Db>` — return a clone of the currently-open
  project's `Db` handle. Fails with `AppError::NoProjectOpen` when
  no project is open. **Every existing command routes SQL through
  this method**, so a "no project" state is a first-class error
  instead of a null pointer.
- `current_project()` / `set_project()` — swap the open project;
  the second also updates `AppSettings.last_project_path` and the
  recent-projects list, and persists both to disk.
- `get_settings()` / `update_settings(f)` — read or mutate the
  persisted `AppSettings` blob.
- `thumb_cache_dir()` — returns the **per-project** subdirectory
  under `thumbs_root` (see [Thumbnail pipeline](#corethumbnailrs)).
- `magnifier_context()` / `set_magnifier_context()` /
  `set_magnifier_current()` — small piece of state stashed by the
  main window before spawning the Magnifier pop-up so the pop-up
  can pull its list context via one command instead of stuffing a
  serialised `ImageFilter` / `ImageSort` into a URL. See
  [Magnifier window](#magnifier-window).

## `core/project.rs` — project abstraction

A **project** = a single `.magpie` SQLite file the user chose the
location of.

Key types:

- `AppSettings` — persisted JSON blob at
  `%APPDATA%\com.magpie.app\app-settings.json`. Holds theme, font
  size, language, last-project path, and recent-projects list
  (bounded to `RECENT_PROJECTS_MAX = 10`).
- `Theme` (`system|dark|light`), `FontSize` (`small|medium|large`).
- `ProjectState { db: Option<Db>, info: Option<ProjectInfo> }` —
  live state guarded by the `project` mutex on `AppServices`.
- `ProjectInfo { path, name }` — DTO handed to the frontend.

Key functions:

- `create_project(path)` — refuses to overwrite existing files.
- `open_project(path)` — opens an existing DB.
- `save_project_as(current, new_path)` — uses SQLite's online
  `rusqlite::backup::Backup` API to copy the DB, then reopens.
- `auto_open_on_startup(app_data_dir, &mut settings)` — resolves
  which project (if any) to open on launch. Priority:
  1. **CLI arg** — if `argv[1]` is an existing `.magpie` file
     (Windows "open with" hands us the path), open that. Also
     updates `last_project_path`.
  2. `settings.last_project_path` if it points at an openable file.
  3. Legacy `%APPDATA%\com.magpie.app\magpie.db` — renamed in place
     to `Default.magpie` and **added to `recent_projects`** but
     **not** auto-opened. The user still sees the welcome screen on
     their first launch after upgrading and can click the migrated
     project to open it. This deliberately gives up "silently
     resume" for one-off migration launches so requirement 5.1
     (buttons visible when no project is open) always holds.
  4. Nothing.

## `menu.rs` — native menu bar

Builds the `Project / Edit / View / Settings` menu via Tauri 2's
`MenuBuilder`. Each item has a stable string ID (`ID_PROJECT_NEW`,
`ID_EDIT_UNDO`, …). Clicks are forwarded to the renderer as
`app://menu` events; the frontend's `useMenuRouter` decides what to
do.

The `set_menu_item_enabled(id, enabled)` command lets the frontend
grey out context-sensitive items. Currently used for:

- `edit_undo` / `edit_redo` — driven by undo/redo stack length.
- `view_magnifier` — driven by `useStore.selection.primary`; the
  menu item ships disabled and is only enabled when the user
  selects a file. Matches the requirement to disable
  "View → Magnifier" when nothing is selected.

## Magnifier window

The magnifier is a **separate native `WebviewWindow`** (label
`"magnifier"`), created from the frontend via
`new WebviewWindow(…)` when the user double-clicks a tile or picks
View → Magnifier. It loads the same bundle as the main window with
`url: "index.html#magnifier"`; `src/main.tsx` inspects
`location.hash` and renders `<MagnifierWindow />` instead of `<App />`
for that route.

Coordination between the two windows lives in `AppServices`:

```rust
pub struct MagnifierContext {
    pub image_id: Option<i64>,
    pub filter: ImageFilter,
    pub sort: ImageSort,
}
```

Flow:

1. Main window calls `set_magnifier_context(image_id, filter, sort)`
   (`commands/magnifier.rs`) before creating the window. This
   stashes the DTO in the mutex.
2. If a magnifier window already exists it's focused and receives an
   `app://magnifier-reset` event so it re-reads the context and
   jumps to the new image.
3. Otherwise `new WebviewWindow` creates it. The popup calls
   `get_magnifier_context()` on mount, queries the same filtered +
   sorted set as the grid, and displays the current image.
4. Arrow-key navigation calls `set_magnifier_current(image_id)` so
   the "last shown" pointer stays fresh.

The magnifier window is scoped in `capabilities/default.json`:

```json
"windows": ["main", "magnifier"],
"permissions": [
  "core:webview:allow-create-webview-window",
  "core:window:allow-close",
  "core:window:allow-set-focus",
  "core:window:allow-set-title",
  ...
]
```

Both windows are on the same origin so IPC, the asset protocol, and
the shared SQLite handle all Just Work.

## `types.rs` — IPC types

- `LibraryFolder` — one registered folder; includes `isAvailable`.
- `ImageSummary`, `ImageDetails` — full and abridged views of an
  image row. `id` is the plain `images.id` primary key of the open
  project's DB.
- `MetadataPatch` — the "what to change" payload for
  `update_image_metadata` and `batch_update_metadata`.
- `double_option` module — custom Serde deserializer for
  `Option<Option<T>>`. Distinguishes "field missing" from
  "field explicitly null" on the wire.

`ProjectInfo`, `AppSettings`, and `AppSettingsPatch` live in
`core::project` and `commands::settings` respectively (both
`Serialize + Deserialize` with camelCase JSON).

Every struct uses `#[serde(rename_all = "camelCase")]` so JSON keys
are camelCase but Rust fields stay snake_case.

## `error.rs`

`AppError` enum (via `thiserror`):

- `Io(std::io::Error)` — filesystem errors.
- `Db(rusqlite::Error)` — DB errors.
- `Pool(String)` — mutex-poisoning errors on the shared `Db`
  handle. (Legacy variant name; kept for compatibility.)
- `NoProjectOpen` — every DB-touching command returns this when no
  project is currently open. The frontend interprets it as "route
  the user to the welcome screen".
- `MetadataRead(String)` — user-facing metadata read failures.
- `Internal(String)` — anything else.

All commands return `AppResult<T>` (= `Result<T, AppError>`).
The `Display` impl for `AppError` is safe to surface to the user
(no absolute paths in strings without context).

## `db/` — single-project storage

See [Database schema](./schema.md) and
[Database design](./db-redesign.md).

- `db/mod.rs` — the `Db` handle wrapping
  `Arc<Mutex<Connection>>` plus `open`, `with_conn`,
  `with_conn_mut`. Defines `DB_FILE_NAME = "magpie.db"` (used only
  as the legacy filename and internal default) and `SCHEMA_VERSION`.
- `db/schema.rs` + `db/schema.sql` — DDL for a fresh DB and the
  version check for an existing one.
- `db/queries.rs` — every query in one file, grouped by concern:
  folders, images (upsert / meta / delete / **rename**),
  MetadataPatch, tag rename/delete, search (`query_images`,
  `list_all_tags`), smart collections. Tag operations are
  **source-aware**:
  - `set_image_meta` (scanner path) inserts `'auto'` rows only
    when the name isn't already carried by the image (never
    deletes).
  - `add_auto_tags_for_image` (automatic AI tagging path,
    `core::auto_tag`) uses the same additive-only rule, so AI
    suggestions merge with XMP-derived auto tags in a single
    read-only bucket.
  - `apply_metadata_patch` (UI path from the details panel) only
    ever touches `'user'` rows.

  See [Schema › image_tags](./schema.md#image_tags) for the split.
- `db/migrate.rs` — startup importer for the two pre-project
  layouts (per-folder `.magpie/library.db` and the earlier central
  `library.db`). Runs the first time the app sees them and
  produces the project's SQLite file.

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

Every DB touch goes through `services.db()?` first, so if the user
closed the project mid-scan the current file simply errors out and
the scan aborts cleanly.

## `core/auto_tag/`

Optional pipeline chained onto the tail of a successful scan when
`AppSettings.aiAutoTag` is on:

- `mod.rs` — `tag_folder(services, app_handle, folder_id)`. Same
  shape as `scanner::scan_folder`: enumerate candidates, run
  per-image work on a semaphore, emit
  `app://auto-tag` progress events (payload
  [`AutoTagProgress`](./commands.md#events)). All AI passes go
  through a single `tokio::sync::Mutex<()>` on
  `AppServices.auto_tag_gate` so multiple newly-added folders queue
  up FIFO instead of thrashing.
- `classifier.rs` — the small `ImageClassifier` trait
  (`classify(bytes) → Vec<TagSuggestion>` + `min_confidence` +
  `max_tags_per_image`) plus the deterministic `MockClassifier`
  used exclusively in tests.
- `clip_classifier.rs` — production `ImageClassifier` backed by
  OpenAI's **CLIP-ViT-B/32**, driven by
  [`candle`](https://github.com/huggingface/candle) (pure-Rust ML;
  no C++ or ONNX Runtime dependency). Per-image work:
  1. `preprocess_image` — decode with `image::load_from_memory`,
     resize the short side to 224, centre-crop 224×224, split into
     3×224×224 NCHW `[0,1]`, then apply CLIP mean/std normalisation.
  2. `ClipModel::get_image_features` on `Device::Cpu` → 512-dim
     embedding, L2-normalised.
  3. Cosine-similarity dot-product against the pre-computed text
     embedding matrix, keep the top-`MAX_TAGS_PER_IMAGE=6` above
     `MIN_COSINE=0.20`.

  Vocabulary is compiled in from
  `core/auto_tag/resources/photo_vocab_v1.txt` (~1000 photo
  words). Text embeddings for that vocab are computed once via
  `compute_text_embeddings` (BPE tokenise with the CLIP tokenizer,
  pad to 77 tokens, run `get_text_features`, L2-normalise) and
  cached under `<app_data_dir>/models/clip/photo_vocab_v1.embeddings.f32`
  keyed by `SHA-256(photo_vocab_v1.txt)` — bumping the vocab file
  invalidates the cache automatically.
- `model_manager.rs` — downloads and verifies the two required
  files under `<app_data_dir>/models/clip/`:
  `model.safetensors` (~605 MB, pinned SHA-256) and `tokenizer.json`
  (~2 MB, trusted). Streams bytes with `reqwest` + `futures-util`
  to a `.part` file, verifies the hash, then atomically renames.
  Publishes `app://ai-model-download` progress events (payload
  [`AiModelDownloadProgress`](./commands.md#events)) throttled to
  200 ms so the dialog can draw a smooth progress bar.

  A few details worth knowing when debugging network failures:

  - `reqwest` is built with the `rustls-tls-native-roots` feature so
    TLS uses the **Windows trust store** (SChannel roots via
    `rustls-native-certs`). This is required when the user sits
    behind a corporate MITM proxy whose CA is not in the bundled
    Mozilla `webpki-roots` set.
  - Downloads are **resumable**. Bytes are hashed as they land in
    the `.part` file, and if a request fails mid-stream the next
    attempt sends `Range: bytes=N-` from wherever the hash counter
    stopped, so a mid-transfer drop costs one round-trip rather
    than 600 MB.
  - Each file is retried up to `MAX_DOWNLOAD_ATTEMPTS = 4` times
    with 2s / 4s / 8s exponential backoff. On a hard failure the
    error message contains the full `reqwest → hyper → rustls`
    source chain (`chain()` helper walks `.source()` recursively),
    which is what the Settings dialog surfaces to the user.
  - Per-request timeouts: 30 s to connect, 30 min for the whole
    body, plus TCP keep-alive every 30 s so stateful firewalls
    don't silently drop the connection during long downloads.
- `tag_folder` refuses to spawn the classifier when
  `model_manager::check_status` reports the safetensors or
  tokenizer files missing — it emits a single "finished with
  error" `AutoTagProgress` so the status bar shows a warning, and
  the Settings dialog is the only path that starts a download.

Per-image bookkeeping lives on the `images` row itself:
`ai_tagged_at` (Unix ms of the last successful pass) and
`ai_tag_hash` (the file's fingerprint — `content_hash` when known,
else `mtime_ms.to_string()`). `queries::list_auto_tag_candidates`
skips rows whose fingerprint still matches, so a rerun on an
unchanged folder is O(candidates) DB reads and no classifier work.
See [Scanner → Auto-tag pass](./scanner.md#stage-7---auto-tag-pass-opt-in)
for the full stage-by-stage description.

## `core/thumbnail.rs`

```rust
pub fn ensure_thumbnails(cache_dir: &Path, src: &Path, image_id: i64) -> Result<()>;
pub fn thumb_path(cache_dir: &Path, image_id: i64, size: ThumbSize) -> PathBuf;
pub fn delete_thumbnails(cache_dir: &Path, image_id: i64);
```

Decode via `image::open`, resize via `fast_image_resize` (SIMD
Lanczos3), encode via `webp::Encoder` at quality 80. `image_id` is
the plain `images.id` primary key.

**Thumbnail cache is scoped per project.** `AppServices` exposes
`thumb_cache_dir()` which returns
`%APPDATA%\com.magpie.app\thumbs\<key>\` where `<key>` is a 64-bit
hash of the currently-open project's absolute path (case-insensitive
on Windows). This guarantees two projects that happen to share the
same `image_id` — SQLite's autoincrement is per-DB — never see each
other's cached previews. `thumb_cache_dir()` returns
`AppError::NoProjectOpen` when nothing is open, and every call site
handles that gracefully.

The hashing function `project::thumb_cache_key(path)` is covered by
unit tests for stability and case handling.

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
`db::queries::apply_metadata_patch` and lives in the project DB.

The single exception is **file renames** via `rename_image` —
`std::fs::rename` on disk followed by `queries::rename_image_row`
in one command. This is the only place Magpie modifies anything
under a source folder, and only when the user explicitly asks.

## `commands/*.rs`

Each file exposes 3–5 `#[tauri::command]` async functions. See
[Tauri command reference](./commands.md) for the full list.

Common patterns:

- `services: State<'_, Arc<AppServices>>` for shared state.
- `let db = services.db()?;` at the top of every DB-touching
  command — surfaces `NoProjectOpen` as a normal error.
- `db.with_conn(...)` / `db.with_conn_mut(...)` for every SQL
  operation.
- `app_handle: AppHandle` for emitting events.
- Return `AppResult<T>`; Serde does the JSON legwork.
- Tracing spans on entry so `app.log` shows every IPC hit.
