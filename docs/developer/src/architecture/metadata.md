# Metadata pipeline

The metadata pipeline is where Magpie most differs from a plain
file browser. It has two sub-pipelines (read + patch) and one hard
invariant that ties them together:

> **Invariant.** For every file the scanner has seen, the
> per-folder `library.db` row is the single source of truth for
> user metadata (title + tags). Magpie **never writes back into the
> source file**. On first scan we read existing tags out of XMP
> and the Windows Shell property store so the user doesn't lose
> anything they'd already labelled; from that point on the DB is
> authoritative.

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

`library::apply_metadata_patch` runs the whole patch in a single
transaction on the *folder's* library DB:

1. Update the `images` row title.
2. If `tags` is set: replace the row's tags.
3. If `tags_add` is set: insert missing.
4. If `tags_remove` is set: delete matching.
5. Rebuild the FTS5 row (DELETE + INSERT).
6. Commit.

If any step fails, the transaction rolls back. Nothing partially
lands.

The batch command (`batch_update_metadata`) uses the packed global
IDs to group edits by folder, then applies the patch inside each
folder's DB in one transaction each — cross-folder writes never
share a transaction so a network share falling over doesn't roll
back edits to a local disk.

## No write-back to source files

There is deliberately no "write pipeline" any more. `FormatHandler`
has no `write_user` method; `win_shell::write_user_meta` is gone;
so is the atomic-write helper and every `build_xmp_packet` call
site. See [Database redesign § What the file bytes see change](../design/db-redesign.md#what-the-file-bytes-see-change).

## FS refresh on read

Every `get_image` call does:

```rust
if fs_meta.mtime > row.mtime_ms {
    let fresh = read_all(&registry, &path);
    library::set_image_meta(&mut conn, local_id, &fresh);
}
```

Simple mtime comparison: if the source file changed after Magpie's
last import, re-read its metadata into the DB. This is how a tag
added in Windows Explorer becomes visible on next click of that
file **only for the first mtime bump after import** — because we
overwrite the DB row from the file. After the first import, Magpie
edits stay in the DB even if the file changes on disk (Magpie
doesn't currently detect a *tag-only* Explorer edit vs. a real
content edit, so `mtime` bumps do wipe Magpie-side tag edits with
the file's current tags — an intentional trade-off; the DB is the
source of truth if you want your edits to stick, external tools
should not be used to tag afterwards).

## Windows Shell property store (import-only)

For formats without a native XMP parser (RAW, HEIC, MP4, PDF, …),
Magpie falls back to Windows' Shell property store to read tags
and titles on first scan. `System.Title`, `System.Keywords`, and
similar canonical properties are consulted. This ensures that a
user who had been tagging RAWs through Explorer's *Properties →
Details* dialog doesn't lose those tags on migration.

`win_shell` was previously bidirectional; after the redesign it is
strictly read-only.
