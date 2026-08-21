# Metadata pipeline

The metadata pipeline is where Magpie most differs from a plain
file browser. It has three sub-pipelines (read, patch, write) and a
single invariant that ties them together:

> **Invariant.** After a successful save via a writable
> [format handler](../design/file-formats.md), the database and the
> embedded metadata inside the source file reflect the same title
> and tags. Magpie does not produce sidecar files; any pre-existing
> legacy sidecar is removed by the write path after the embed
> succeeds. For files whose handler is read-only, the DB row and the
> library are the sole source of truth for tags.

## The read pipeline

```
       ┌───────────────────────┐
       │  read_all(registry,   │
       │           path)       │
       └──────────┬────────────┘
                  │
    ┌─────────────┴──────────────┐
    ▼                             ▼
┌────────────────┐          ┌──────────────────┐
│ handler        │          │ handler          │
│ .read_technical│          │ .read_user       │
│ (path)         │          │ (path)           │
└──────┬─────────┘          └──────┬───────────┘
       │                            │
       ▼                            ▼
(dimensions,                  (title, tags)
 taken_at,                          │
 camera,                            │
 duration…)                         │
       │                            │
       │       ┌────────────────────┘
       │       │
       │       ▼
       │  legacy sidecar (only if present)
       │       │
       └───────┴──────► Merged ImageMetaFromFile
```

Ordering within user meta: the handler's own `read_user` is called
first; then any **legacy** sidecar (from an older Magpie version or
from Lightroom) is parsed and its non-empty fields overwrite the
handler's values. This preserves user data during the one-way
migration to embed-only. The next successful save embeds the merged
state and removes the sidecar.

The XMP parser (`quick_xml` state machine in
`core/formats/xmp_packet.rs`) handles both the Adobe standard fields
and Microsoft-Explorer variants:

- `dc:title`, `dc:description`, `dc:subject` (Alt/Bag containers)
- `xmp:Rating` (attribute or element — read for preservation, not
  surfaced in the UI)
- `MicrosoftPhoto:LastKeywordXMP` (Windows Explorer's tag store)
- Attribute-only forms (some tools flatten Alt into an attribute)

## The patch pipeline

Frontend produces a `MetadataPatch` — a struct where every field is
`Option`-wrapped so the caller can express "don't touch" vs.
"clear" vs. "set":

```rust
pub struct MetadataPatch {
    pub title: Option<Option<String>>,
    pub tags: Option<Vec<String>>,      // whole list, replaces
    pub tags_add: Option<Vec<String>>,  // additive
    pub tags_remove: Option<Vec<String>>,
}
```

`apply_metadata_patch` (in `db/queries.rs`) runs the whole patch in
a single transaction:

1. Update the `images` row title.
2. If `tags` is set: replace the row's tags.
3. If `tags_add` is set: insert missing.
4. If `tags_remove` is set: delete matching.
5. Rebuild the FTS5 row (DELETE + INSERT).
6. Commit.

If any step fails, the transaction rolls back. Nothing partially
lands.

## The write pipeline

After `apply_metadata_patch` returns success, the caller
(`update_image_metadata` or `batch_update_metadata` via
`apply_patch_and_persist`) does:

```
   ┌────────────────────────────────────────┐
   │  Fetch the *final* state via get_image │
   │  (reflects the just-applied patch)     │
   └───────────────┬────────────────────────┘
                   ▼
   ┌────────────────────────────────────────┐
   │  spawn_blocking →                      │
   │  write_metadata_to_source              │
   │  ├─ registry.for_ext(ext) → handler    │
   │  ├─ handler.write_user(path, meta)     │
   │  │    JPEG   → APP1 XMP                │
   │  │    PNG    → iTXt XMP chunk          │
   │  │    WebP   → RIFF XMP chunk          │
   │  │    GIF89a → Application Extension   │
   │  │    else   → Err(unsupported)        │
   │  └─ delete legacy .xmp if any          │
   └───────────────┬────────────────────────┘
                   ▼
   ┌────────────────────────────────────────┐
   │  On Ok:                                │
   │  set_meta_written_at + set_meta_read_at│
   │  Emit app://image-updated              │
   │                                        │
   │  On Err (writable handler failed):     │
   │  Propagate to caller (UI toast).       │
   │                                        │
   │  On Err (handler is read-only):        │
   │  DB kept, UI shows "library only" note.│
   └────────────────────────────────────────┘
```

Three design choices worth calling out:

1. **The XMP packet is built from the DB's final state, not from
   the patch.** This way, "add tag X" in batch mode writes a packet
   containing every tag currently on the file, not just X.
2. **`meta_read_at` is bumped on write** so the FS-refresh check in
   `get_image` doesn't fire on files Magpie itself just wrote.
3. **File-write failure surfaces to the UI.** Unlike the old
   "sidecar is the fallback" design, there's no silent fallback: an
   unsupported format returns `Err` so the user sees a clear note.
   Any tag entered by the user is still recorded in the DB either
   way, so nothing they typed is lost.

## FS refresh on read

Every `get_image` call does:

```rust
if refresh_needed_from_fs(&path, &cached) {
    let fresh = read_all(&registry, &path);
    resync_user_meta_from_fs(&db, id, &fresh);
    set_meta_read_at_now(&db, id);
}
```

`refresh_needed_from_fs` is a simple mtime comparison: if the
source file (or any legacy `.xmp` alongside it) was modified after
Magpie's last read, we re-read from disk. This is how a tag added
in Windows Explorer becomes visible on next click of that file.
The legacy-sidecar branch remains so that users migrating from
Lightroom (or an older Magpie) still get their existing edits on
first scan.

## Windows Explorer tag interop specifics

Explorer writes tags into JPEGs using both `dc:subject` (standard)
and `MicrosoftPhoto:LastKeywordXMP` (Microsoft-specific). Some
older workflows only write the latter, so Magpie's reader accepts
both and unions them, deduping case-insensitively.

On the write side, Magpie emits `dc:subject` in the XMP packet.
Windows Explorer resolves its *Tags* column from either
`dc:subject` or `MicrosoftPhoto:LastKeywordXMP`, so a Magpie-written
JPEG shows up correctly in Explorer with only the standard Dublin
Core block.

## Preserving fields Magpie doesn't own

The XMP builder preserves any `dc:description`, `xmp:Rating`, GPS
coordinates, and other tags a foreign tool wrote — Magpie's UI
doesn't expose these, but the reader still parses them into
`XmpUserMeta` so the writer can put them back into the rebuilt
packet unchanged. See the `write_preserves_foreign_rating_and_description`
integration test.
