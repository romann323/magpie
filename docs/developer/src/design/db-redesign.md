# Database design

Magpie stores every piece of user-visible metadata in a single SQLite
file at `%APPDATA%\com.magpie.app\magpie.db`. Source files on disk
are never modified.

This chapter captures the rationale, the schema shape, and the
migration story from the two earlier on-disk layouts that Magpie has
carried users through.

## Why a single central DB

We evaluated three options:

1. **Central DB in `%APPDATA%`** — the current design.
2. **Per-folder DB inside `<folder>/.magpie/library.db`** — the
   previous design.
3. **Metadata written back into source files** — the pre-redesign
   design.

Option 3 was abandoned because too many formats (RAW, PDF, MP4,
HEIC, ...) don't accept sidecar-free tag writes on Windows without
elevation and format-specific tricks, and the roundtrip cost was
crushing scan performance.

Option 2 solved the write problem but introduced its own set of
issues:

- Hidden `.magpie` folders inside the user's photo trees.
- Cloud-sync clients (OneDrive, Dropbox, iCloud) racing against
  Magpie for the DB file.
- Cross-folder search required attaching every registered library to
  a single connection at startup, with a hard cap at 10 attached DBs
  in the bundled SQLite.
- Packed *global* IDs (`folder_id * 1_000_000_000 + local_id`) to
  paper over the fact that each folder's `images.id` starts at 1.

Option 1 keeps the "read-only source files" invariant from option 2
but drops every one of its downsides:

- One file. Trivial to back up, trivial to reason about.
- Lives outside the user's photo trees.
- Cloud-sync clients don't touch `%APPDATA%` by default, so the
  race disappears.
- Cross-folder search is a plain `SELECT ... FROM images` with no
  attach dance.
- `images.id` is a normal autoincrement PK and is globally unique
  for free.

Trade-off: the DB doesn't travel with a folder. If the user moves a
photo folder to another PC, they need to add it fresh on the target
machine and re-tag (or re-scan to pick up whatever tags the files
already embed). This matches how every other photo manager on the
market works.

## Schema

The full DDL lives in
[`src-tauri/src/db/schema.sql`](../../../../src-tauri/src/db/schema.sql).
See the [Database schema](./schema.md) chapter for column-by-column
tables. Highlights:

- `library_folders` — registered folder roots, absolute canonical
  paths.
- `images` — one row per scanned file, `(folder_id, rel_path)`
  UNIQUE, `folder_id` FK to `library_folders` with ON DELETE
  CASCADE.
- `tags` — global vocabulary; `name` UNIQUE COLLATE NOCASE so
  case variants merge automatically.
- `image_tags` — many-to-many join, both FKs cascade on delete.
- `images_fts` — FTS5 over `title`, `filename`, `tags`.
- `smart_collections`, `app_settings`, `schema_meta` — small
  bookkeeping tables.

### Paths

`images.rel_path` is folder-relative with forward slashes. Absolute
paths are built at query time by joining with `library_folders.path`
in Rust. Moving a folder root only requires updating one row in
`library_folders`.

### IDs

`images.id` is a plain SQLite autoincrement `INTEGER PRIMARY KEY`.
It's globally unique because there's only one central DB. The IPC
layer forwards it to the frontend as `ImageSummary.id` verbatim;
JavaScript's `Number.MAX_SAFE_INTEGER` gives us 2⁵³ − 1 values,
which is nine quadrillion images — more than enough.

## Connection layer

`crate::db::Db` wraps a single `rusqlite::Connection` behind
`Arc<Mutex<Connection>>` for `Send + Sync`. Every command goes
through one of two shortcuts:

- `db.with_conn(|conn| …)` — read/write borrow for a single
  statement or read-only query.
- `db.with_conn_mut(|conn| …)` — the closure receives `&mut
  Connection` so it can start a transaction.

The mutex is only held for the closure's lifetime; long-running
scans and thumbnail generation run outside it and touch the DB in
short bursts.

## Read-only source files

Format handlers implement two methods: `read_technical` (returns the
label/value pairs shown in the "File info" section of the details
panel) and `read_user` (returns title + tags). There is no
`write_user`. On first scan the scanner reads whatever the file
already carries — native XMP for JPEG/PNG/WebP/GIF, the Windows
Shell property store for everything else, plus legacy Lightroom
`.xmp` sidecars — and imports it into `magpie.db`. From that point
on the DB is the sole source of truth; the file itself is left
untouched.

## Migration from earlier layouts

`db::migrate::open_or_migrate(app_data_dir)` runs on every launch:

1. **`magpie.db` exists** — open it, apply any pending schema
   migrations, done.
2. **`registry.db` exists (per-folder design)** — create a fresh
   `magpie.db`, then for each row in the old registry:
   - Read `<folder>\.magpie\library.db` read-only.
   - Copy the folder row into `magpie.db.library_folders` (new PK).
   - Copy every image row, remapping `folder_id`.
   - Copy tags via name (insert-or-ignore into the central `tags`
     table) and rebuild `image_tags`.
   - Rebuild the FTS row for each imported image.
   - After all folders succeed, rename `registry.db` to
     `registry.db.migrated-<yyyymmddThhmmss>` and delete each
     migrated `<folder>\.magpie` directory.
3. **`library.db` exists (pre-redesign)** — same idea, but the
   source is a single old-central DB. Absolute paths in the legacy
   `images.path` column are converted to folder-relative using
   `library_folders.path`. Legacy file is renamed to
   `library.db.migrated-<ts>`.
4. **Nothing exists** — create empty `magpie.db`.

Migration is idempotent: `magpie.db` is populated first, source
files are only renamed after everything commits. A mid-migration
crash leaves the legacy files untouched and the next launch retries
from step 1.

## What the redesign removed

Rust:

- `db/pool.rs` — the `LibraryPool` and its ATTACH management.
- `db/library.rs` / `db/registry.rs` — schema modules for the
  per-folder and registry DBs; consolidated into `db/queries.rs`.
- `db/search.rs` — the `UNION ALL`-across-attached-schemas query
  builder; replaced by a plain `SELECT ... FROM images`.
- `db/legacy_migration.rs` — replaced by `db/migrate.rs` which
  handles both legacy layouts.
- `db::pack_global_id` / `db::unpack_global_id`.
- `commands::library::check_folder_sync_risk` and
  `SyncRiskWarning` — with the DB in `%APPDATA%`, cloud sync on the
  photo folder is safe.

TypeScript:

- `SyncRiskWarning` type + `checkFolderSyncRisk` IPC wrapper.
- The `confirm` dialog in `TopBar.tsx` that gated adding folders on
  cloud drives.

## Diagnostics

Two diagnostic examples are shipped:

- `cargo run -q --example dump_meta` — dumps the first few rows
  from `magpie.db` and re-runs the metadata read pipeline on each.
- `cargo run -q --example dump_tag_usage` — prints the images
  associated with a tag (`$env:MAGPIE_QUERY_TAG = 'sunset'`).

Both open `magpie.db` read-only and are safe to run while Magpie is
running.
