# Out of scope for v1

Explicitly deferred to keep v1 shippable. Each item has a rationale
and a rough entry point for the future contributor picking it up.

## Photo editing

- **No crop, rotate, exposure, or colour correction.** Magpie treats
  pixels as read-only. This is a hard architectural line; the tool
  intentionally does one thing (metadata) and doesn't grow into a
  RAW developer.

## Face and object recognition

- **No auto-tagging based on ML models.** The privacy story is
  simpler when no images ever run through an ML pipeline. Future
  work could offer optional local inference (e.g. running an ONNX
  model against a batch of photos) with a clear opt-in flow.

## Cloud sync

- **No account, no cloud storage integration.** OneDrive, Dropbox,
  Google Drive, and iCloud are third-party sync backends the user
  configures at the OS level; Magpie reads whatever the OS has
  materialised, and warns when a picked folder lives on such a
  disk (see `check_folder_sync_risk`).

## Writing tags into source files

- **Magpie deliberately does not write into source files.** After
  the DB redesign, tags and titles live exclusively in each
  folder's `.magpie/library.db`. This removes an entire category
  of bugs (partial writes, format-specific edge cases, cloud sync
  re-uploading gigabytes for one tag change) at the cost of losing
  the "tag once, portable everywhere in the XMP ecosystem" story.
  Users who need file-embedded tags can still author them in
  Lightroom / Bridge / Explorer; Magpie will pick them up on the
  first scan of that folder.

## Ratings, colour labels, pick / reject flags

- No UI for star ratings, `xmp:Label` colour labels, or pick /
  reject flags. The DB schema could grow columns for them; the
  read-only XMP parser already understands `xmp:Rating`.

## In-app full-screen preview

- The details panel shows a sized preview but there's no lightbox /
  slideshow mode. Would be a new React route reading from
  `getImagePath`.

## Undo / redo

- No global undo stack. The safety net is auto-save + Recycle Bin +
  the fact that source files are never modified — the user's raw
  material is never lost, but reverting a DB edit means editing
  again.

## Smart collections editor

- The DB schema and Tauri commands for smart collections exist
  (they live in the central `registry.db`), but the UI doesn't
  expose creation/editing yet.

## Duplicate detection

- Content hashes are computed on scan and stored (`content_hash`
  column in each library DB), but no duplicate-finder UI ships in
  v1. A future task could add a "Duplicates" filter that joins
  across attached DBs on `content_hash`.

## Multi-PC concurrent editing on a synced folder

- Magpie *warns* when a folder lives on OneDrive / Dropbox /
  Google Drive / iCloud / UNC share, but doesn't currently
  coordinate edits between two PCs that open the same
  `library.db` at once. Safe usage is "one PC at a time".
