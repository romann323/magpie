# Metadata pipeline

The metadata pipeline is where Magpie most differs from a plain
file browser. It has two sub-pipelines (read + patch) and one hard
invariant that ties them together:

> **Invariant.** For every file the scanner has seen, the row in
> `magpie.db` is the single source of truth for user metadata
> (title + tags). Magpie **never writes back into the source file**.
> Tags stored in `image_tags` carry a `source` — `'auto'` for the
> ones Magpie read out of XMP / Windows Shell / sidecars, `'user'`
> for the ones typed inside Magpie's DetailsPanel. The two are
> managed independently; see [Schema › image_tags](../design/schema.md#image_tags).

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
       │  Windows Shell IPropertyStore (Win-only fallback)
       │       │
       └───────┴──────► Merged ImageMetaFromFile
```

Format-native `read_user` runs first; then any Windows-specific
Shell property read runs to catch formats we don't natively parse
(RAW, HEIC, MP4, PDF, …). Legacy `.xmp` sidecars are also read at
this stage for backward compatibility with older Magpie versions
and Lightroom projects.

The XMP parser (`quick_xml` state machine in
`core/formats/xmp_packet.rs`) handles both the Adobe standard
fields and Microsoft-Explorer variants:

- `dc:title`, `dc:subject` (Alt / Bag containers)
- `MicrosoftPhoto:LastKeywordXMP` (Windows Explorer's tag store)
- Attribute-only forms (some tools flatten `Alt` into an attribute)

`read_all` is called by the scanner on first sight of a file, and
by `get_image` if the file's `mtime` has moved forward since last
import.

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

`queries::apply_metadata_patch` runs the whole patch in a single
transaction on `magpie.db`. **All tag operations target the `'user'`
source**; auto rows are never inserted, removed, or renamed here.

1. Update the `images` row title.
2. If `tags` is set: replace the row's **user** tags (auto rows
   untouched).
3. If `tags_add` is set: insert missing **user** rows.
4. If `tags_remove` is set: delete matching **user** rows.
5. Rebuild the FTS5 row (DELETE + INSERT, `SELECT DISTINCT` over
   both sources so a name present in both isn't indexed twice).
6. Commit.

If any step fails, the transaction rolls back. Nothing partially
lands.

The batch command (`batch_update_metadata`) loops over each ID and
applies the patch inside its own transaction. A failure on one image
doesn't roll back the others; the command reports which IDs
succeeded.

## No write-back to source files

There is deliberately no "write pipeline" any more. `FormatHandler`
has no `write_user` method; `win_shell::write_user_meta` is gone;
so is the atomic-write helper and every `build_xmp_packet` call
site. See [Database design](../design/db-redesign.md).

## FS refresh on read

Every `get_image` call does:

```rust
if fs_meta.mtime > row.mtime_ms {
    let fresh = read_all(&registry, &path);
    queries::set_image_meta(&mut conn, id, &fresh);
}
```

Simple mtime comparison: if the source file changed after Magpie's
last import, re-read its metadata into the DB via
`queries::set_image_meta`. That path is **additive-only** for tags:
each name the file currently reports is inserted as `'auto'` unless
the image already carries it in either source; nothing is ever
deleted. As a result:

- A tag added in Windows Explorer / Lightroom between scans shows up
  as a new automatic tag on next click of the file.
- A tag **removed** from the file between scans stays in the DB
  (still visible under Automatic tags in the details panel) —
  we can't tell the difference between "file lost the tag" and
  "file's own metadata is stale", and the safer default is to keep
  it.
- Tags the user typed inside Magpie are always preserved across
  rescans, regardless of what the file's own metadata says.

## Windows Shell property store (import-only)

For formats without a native XMP parser (RAW, HEIC, MP4, PDF, …),
Magpie falls back to Windows' Shell property store to read tags
and titles on first scan. `System.Title`, `System.Keywords`, and
similar canonical properties are consulted. This ensures that a
user who had been tagging RAWs through Explorer's *Properties →
Details* dialog doesn't lose those tags on migration.

`win_shell` was previously bidirectional; after the redesign it is
strictly read-only.
