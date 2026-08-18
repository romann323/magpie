# User personas and flows

## Personas

### 1. The prosumer photographer (primary)

Owns 20 000–200 000 photos across multiple external drives.
Shoots JPEG+RAW; already uses Lightroom for developing but wants a
lighter tool for triage, tagging, and rating. Needs bulk operations
to work reliably on thousands of photos at once. Cares deeply about
metadata surviving future tool changes.

### 2. The family archivist (secondary)

Has 30 years of family photos in one big folder tree. Wants to add
who's-in-the-photo tags, rate the keepers, and be able to find
"Christmas 2011" quickly. Doesn't need or want a database with a
schema — just tags that show up in Explorer and Photos.

### 3. The technical developer using PicOrg on other people's libraries

Uses PicOrg as a testbed for interop with XMP-writing tools. Reads
sidecars and embedded XMP with `exiftool` to verify PicOrg's writes.
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
                       ┌────────────────┼─────────────────────┐
                       ▼                ▼                     ▼
                Apply DB patch    Write sidecar         Embed XMP in JPEG
                       │                │                     │
                       └────────┬───────┴─────────────────────┘
                                ▼
                     Update meta_read_at / meta_written_at
                                │
                                ▼
              Emit "image-updated" event; frontend refreshes
```

Success criteria:

- No user input is ever lost to a debounce race.
- A failure in any write step leaves the DB in a consistent state.
- Windows Explorer sees the new tag/title/rating after refresh.

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
                    for each id (sequential):                Per-image failures
                       apply DB patch                        are logged and
                       write sidecar + embed XMP             surfaced to user
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
          get_image checks: sidecar_mtime > meta_read_at?
                       │  yes
                       ▼
          read_all() re-reads XMP + sidecar
                       │
                       ▼
          resync_user_meta_from_fs updates DB
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
                     Move file to        Move sidecar     Delete thumbs
                     Recycle Bin         to Recycle Bin
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
- The user can restore from the Recycle Bin and get the tags/rating
  back on next rescan.
