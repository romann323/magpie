# Out of scope for v1

Explicitly deferred to keep v1 shippable. Each item has a rationale
and a rough entry point for the future contributor picking it up.

## Photo editing

- **No crop, rotate, exposure, or colour correction.** Magpie treats
  pixels as read-only. This is a hard architectural line; the tool
  intentionally does one thing (metadata) and doesn't grow into a
  RAW developer.

## Face and object recognition

- **Automatic object tagging now ships in scope** — an opt-in,
  fully-local pipeline backed by OpenAI **CLIP-ViT-B/32** running
  on the CPU via [`candle`](https://github.com/huggingface/candle).
  See [Scanner → Auto-tag pass](../design/scanner.md) and
  `core::auto_tag` (`clip_classifier` / `model_manager`). The
  `ImageClassifier` trait is what to implement if you want to swap
  in a different backbone (larger CLIP, RAM++, an open-vocab
  detector), and the model files themselves live under
  `<app_data_dir>/models/clip/` — pin new SHA-256 values in
  `model_manager::REQUIRED_FILES` and bump `VOCAB_VERSION` when
  the vocabulary changes.
- Face recognition specifically is still out of scope — the trait
  doesn't currently expose bounding boxes and the DB has no notion
  of a "person" separate from a tag.

## Cloud sync

- **No account, no cloud storage integration.** OneDrive, Dropbox,
  Google Drive, and iCloud are third-party sync backends the user
  configures at the OS level; Magpie just reads whatever the OS has
  materialised. Because the central `magpie.db` lives in
  `%APPDATA%\Roaming` (which Windows doesn't sync by default), the
  database isn't racing any sync client.

## Writing tags into source files

- **Magpie deliberately does not write into source files.** Tags
  and titles live exclusively in `magpie.db`. This removes an entire
  category of bugs (partial writes, format-specific edge cases,
  cloud sync re-uploading gigabytes for one tag change) at the cost
  of losing the "tag once, portable everywhere in the XMP ecosystem"
  story. Users who need file-embedded tags can still author them in
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
  (they live in `magpie.db`), but the UI doesn't expose
  creation/editing yet.

## Duplicate detection

- Content hashes are computed on scan and stored (`content_hash`
  column in `images`), but no duplicate-finder UI ships in v1. A
  future task could add a "Duplicates" filter grouping by
  `content_hash`.

## Multi-PC library sharing

- There is no cross-PC sync of the library database. Each PC keeps
  its own `magpie.db` under `%APPDATA%`; tags don't roam. A future
  task could add explicit export/import of the DB (or a portable
  library mode that puts the DB next to the photos again — the
  earlier per-folder design still exists in the codebase's history
  if a contributor wants to resurrect it as an option).
