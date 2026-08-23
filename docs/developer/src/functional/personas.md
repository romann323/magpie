# User personas and flows

## Personas

### 1. The prosumer photographer (primary)

Owns 20 000–200 000 photos across multiple external drives.
Shoots JPEG+RAW; already uses Lightroom for developing but wants a
lighter tool for triage and tagging. Needs bulk operations to work
reliably on thousands of files at once. Cares deeply about metadata
surviving future tool changes.

### 2. The family archivist (secondary)

Has 30 years of family photos and videos in one big folder tree.
Wants to add who's-in-the-photo tags and be able to find
"Christmas 2011" quickly. Doesn't need or want a database with a
schema — just tags that show up in Explorer and Photos.

### 3. The technical developer using Magpie on other people's libraries

Uses Magpie as a testbed for interop with XMP-writing tools. Reads
embedded XMP with `exiftool` to verify Magpie's writes.
Contributes patches.

## Core flows

### F1. First-time onboarding

```
   ┌─────────┐   Add folder   ┌────────┐  Scan (async)  ┌────────┐
   │ Launch  │───────────────▶│ Picker │───────────────▶│  Grid  │
   └─────────┘                └────────┘                 └───┬────┘
                                                             │
                                                     Live progress
                                                     in status bar
```

Success criteria:

- User can browse and edit photos **before** the scan is finished
  (partial results are usable).
- Progress is visible and cancelable.
- The app never blocks or freezes during scanning.

### F2. Single-photo edit

```
   Click tile ─▶ SingleDetails loads ─▶ User types title
                                        │
                                        ▼ debounced 600ms
                     update_image_metadata IPC (Rust command)
                                        │
                                        ▼
                    Unpack global ID → (folder_id, local_id)
                                        │
                                        ▼
                    library::apply_metadata_patch inside the
                    folder's library.db (single transaction:
                    update title / tags / image_tags, rebuild
                    FTS row, commit).
                                        │
                                        ▼
              Emit "image-updated" event; frontend refreshes.
              The source file is NOT touched.
```

Success criteria:

- No user input is ever lost to a debounce race.
- A failure in the DB write leaves the row exactly as it was
  before (transactional rollback).
- The source file's bytes remain byte-for-byte identical to what
  the camera / editor originally produced.

### F3. Multi-select tag application

```
   Ctrl+click N tiles ─▶ MultiDetails renders ─▶ User types tags
                                                  │
                                                  ▼
                                            Click "Apply"
                                                  │
                                                  ▼
                      batch_update_metadata(ids, {tagsAdd, tagsRemove})
                                                  │
                              ┌───────────────────┴──────────────────┐
                              ▼                                      ▼
              Group by folder_id, then for each                 Per-image failures
              folder: apply patch inside one txn                are logged and
              on that folder's library.db.                      surfaced to user
                              │                                      │
                              └────────────┬─────────────────────────┘
                                           ▼
                                Return list of succeeded ids
                                           │
                                           ▼
                        Frontend invalidates image + tag caches;
                        toast "Updated N images" (or "Save failed").
```

Success criteria:

- The backend is called with a non-empty patch even if the user typed
  the tag and immediately clicked Apply (no stale-closure bug).
- Partial success is a first-class outcome, not an error.
- Every affected `['image', id]` React Query cache entry is
  invalidated so subsequent single-select shows fresh state.

### F4. Rescan and detect external edits

```
        User edits tag in Windows Explorer
                       │
                       ▼
               File's mtime changes
                       │
   (User re-selects photo, or clicks Rescan)
                       │
                       ▼
          get_image checks: file mtime > row.mtime_ms?
                       │  yes
                       ▼
          read_all() re-reads embedded XMP + Shell keywords
                       │
                       ▼
          library::set_image_meta writes the fresh row into
          the folder's library.db
                       │
                       ▼
                    UI refreshes
```

Success criteria:

- External edits are detected without a full rescan.
- The FS is the source of truth; the DB is a cache.

### F5. Delete with safety net

```
   Select photo(s) ─▶ Delete key / button ─▶ Confirm dialog
                                              │
                                        Yes ──┴── No ▶ no-op
                                              │
                                              ▼
                        delete_images(ids, permanent=false)
                                              │
                              ┌───────────────┼───────────────┐
                              ▼               ▼               ▼
                     Move file to        Best-effort delete   Delete thumbs
                     Recycle Bin         of any legacy .xmp
                              │               │               │
                              └───────┬───────┴───────────────┘
                                      ▼
                         Remove image row from DB
                                      │
                                      ▼
                        Emit "images-deleted" event
```

Success criteria:

- A failure at any step for one file doesn't abort the batch.
- The DB row is removed only after the file is safely in the Recycle
  Bin.
- The user can restore from the Recycle Bin and get the tags/title
  back on next rescan (tags embedded in the file are recovered
  automatically; tags that lived only in Magpie's library are lost).
