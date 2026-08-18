# Metadata pipeline

The metadata pipeline is where PicOrg most differs from a plain
file browser. It has three sub-pipelines (read, patch, write) and a
single invariant that ties them together:

> **Invariant.** After a successful save, the database, the sidecar,
> and (when applicable) the embedded XMP inside the source file all
> reflect the same values.

## The read pipeline

```
       ┌───────────────────────┐
       │  read_all(path)       │
       └──────────┬────────────┘
                  │
    ┌─────────────┴──────────────┐
    ▼                             ▼
┌────────┐                 ┌────────────┐
│ EXIF   │                 │ XMP        │
│ read   │                 │ (embedded  │
│        │                 │  + sidecar)│
└───┬────┘                 └──────┬─────┘
    │                             │
    ▼                             ▼
 (taken_at,                 (title,
  camera,                    rating,
  width,                     comment,
  height)                    tags)
    │                             │
    └──────────────┬──────────────┘
                   ▼
        Merged UserMetadata + FileMeta
```

Ordering within XMP: embedded XMP is parsed first, then sidecar is
parsed and its non-empty fields overwrite the embedded ones. This
matches the "sidecar is the latest edit" convention.

The XMP parser (`quick_xml` state machine in `core/metadata/xmp.rs`)
handles both the Adobe standard fields and Microsoft-Explorer
variants:

- `dc:title`, `dc:description`, `dc:subject` (Alt/Bag containers)
- `xmp:Rating` (attribute or element)
- `MicrosoftPhoto:LastKeywordXMP` (Windows Explorer's tag store)
- Attribute-only forms (some tools flatten Alt into an attribute)

## The patch pipeline

Frontend produces a `MetadataPatch` — a struct where every field is
`Option`-wrapped so the caller can express "don't touch" vs.
"clear" vs. "set":

```rust
pub struct MetadataPatch {
    pub title: Option<Option<String>>,
    pub rating: Option<Option<i64>>,
    pub comment: Option<Option<String>>,
    pub tags: Option<Vec<String>>,      // whole list, replaces
    pub tags_add: Option<Vec<String>>,  // additive
    pub tags_remove: Option<Vec<String>>,
}
```

`apply_metadata_patch` (in `db/queries.rs`) runs the whole patch in
a single transaction:

1. Update the `images` row (title, rating, comment).
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
`apply_patch_and_write_sidecar`) does:

```
   ┌────────────────────────────────────────┐
   │  Fetch the *final* state via get_image │
   │  (reflects the just-applied patch)     │
   └───────────────┬────────────────────────┘
                   ▼
   ┌────────────────────────────────────────┐
   │  build_xmp_packet(final_state)         │
   └───────────────┬────────────────────────┘
                   ▼
   ┌────────────────────────────────────────┐
   │  spawn_blocking → write_sidecar        │
   │    (atomic: write .tmp, rename)        │
   └───────────────┬────────────────────────┘
                   ▼
   ┌────────────────────────────────────────┐
   │  embed_xmp_in_source                   │
   │    JPEG → inject APP1                  │
   │    other  → skip (sidecar is enough)   │
   └───────────────┬────────────────────────┘
                   ▼
   ┌────────────────────────────────────────┐
   │  set_meta_written_at + set_meta_read_at│
   │  Emit picorg://image-updated           │
   └────────────────────────────────────────┘
```

Two design choices worth calling out:

1. **The XMP packet is built from the DB's final state, not from
   the patch.** This way, "add tag X" in batch mode writes a sidecar
   containing every tag currently on the photo, not just X.
2. **`meta_read_at` is bumped on write** so the FS-refresh check in
   `get_image` doesn't fire on files PicOrg itself just wrote.

## FS refresh on read

Every `get_image` call does:

```rust
if refresh_needed_from_fs(&path, sidecar_mtime, meta_read_at) {
    let fresh = read_all(&path);
    resync_user_meta_from_fs(&db, id, &fresh);
    set_meta_read_at_now(&db, id);
}
```

`refresh_needed_from_fs` is a simple mtime comparison: if the
sidecar or the source file was modified after PicOrg's last read,
we re-read from disk. This is how a tag added in Windows Explorer
becomes visible on next click of that photo.

## Windows Explorer tag interop specifics

Explorer writes tags into JPEGs using both `dc:subject` (standard)
and `MicrosoftPhoto:LastKeywordXMP` (Microsoft-specific). Some
older workflows only write the latter, so PicOrg's reader accepts
both and unions them, deduping case-insensitively.

On the write side, PicOrg produces both `dc:subject` and
`MicrosoftPhoto:LastKeywordXMP` blocks in the same XMP packet so
Explorer's Tag column shows the tag as if Explorer had written it
itself.
