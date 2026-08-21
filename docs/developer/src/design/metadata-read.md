# Metadata read path

## Entry point

```rust
pub fn read_all(registry: &FormatRegistry, path: &Path) -> AppResult<ImageMetaFromFile>;
```

Called from:

- The scanner (initial scan and rescans), to populate the DB.
- `get_image` (from a Tauri command), when a FS-refresh check
  detects that the source file (or any legacy sidecar) has changed
  since the DB last saw it.

`ImageMetaFromFile` is the union of everything we can pull from disk
that the DB stores:

```rust
pub struct ImageMetaFromFile {
    pub title:        Option<String>,
    pub tags:         Vec<String>,      // deduped, in insertion order
    pub taken_at:     Option<i64>,      // ms since Unix epoch
    pub camera_make:  Option<String>,
    pub camera_model: Option<String>,
    pub width:        Option<u32>,
    pub height:       Option<u32>,
}
```

Note: `rating` and `comment` were dropped in migration 0003; they are
no longer surfaced in the DB or in `ImageDetails`. The XMP builder
still preserves any `xmp:Rating` / `dc:description` a foreign tool
wrote — see [Metadata write path](./metadata-write.md).

## Stages

### 1. Ask the handler

```rust
let handler = registry.for_ext(ext).ok_or(AppError::UnsupportedFormat)?;
let user      = handler.read_user(path)?;
let technical = handler.read_technical(path);
```

`read_user` returns the editable surface (title + tags). `read_technical`
returns the ordered `TechnicalMeta` list used by the UI and by the
scanner for dimensions / EXIF-derived fields (camera, taken_at). See
[File formats](./file-formats.md).

Each handler decides how to fetch this. All writable handlers use the
shared `xmp_packet::parse_xmp` helper; read-only stubs typically only
implement `read_technical` and return an empty `UserMeta` from
`read_user`.

### 2. Legacy sidecar XMP

```rust
let sidecar = read_sidecar(&sidecar_path_for(path)).ok();
```

Sidecar path is `<file basename>.xmp`. Sidecars are a **legacy
compatibility** path: Magpie no longer produces them, but older
Magpie installs and other tools (Lightroom, digiKam) did, and users
would lose data if we ignored them. If a sidecar file exists it's
parsed with the same XMP parser used for embedded packets.

### 3. Merge

`apply_user_meta` is called first with the handler's `UserMeta` (if
any), then with the sidecar XMP (if any). The later call overwrites
any field the earlier call set. Result: **sidecar wins over
embedded**.

Rationale: the sidecar (if it still exists) is by definition older
than Magpie's current write path, but on first scan after upgrade
we must respect edits already stored there. The precedence rule
becomes moot after the first save into the source file, because
`write_metadata_to_source` deletes the sidecar as part of its
cleanup step.

### 4. Technical fields folded in

The scanner reads `technical` entries whose keys match well-known
labels (`Dimensions`, `Taken`, `Make`, `Model`) and folds them into
`ImageMetaFromFile` for storage in the DB. The rest of the list is
purely display-time; it isn't persisted.

## XMP parser

`xmp_packet::parse_xmp(bytes) -> XmpUserMeta` is a hand-written
streaming parser built on `quick_xml`. Uses a simple state machine
and normalises casing/namespaces because tools in the wild are
notoriously inconsistent (I've seen `<X:XMPMETA>`, `<x:xmpmeta>`,
and mixed-case in the same file). See the
`read_sidecar_case_variants` test.

Recognises both `dc:subject` (standard) and
`MicrosoftPhoto:LastKeywordXMP` (Windows Explorer); unions the two
sets, deduping case-insensitively while preserving the first
observed casing. Handles both `<rdf:Alt>` and `<rdf:Seq>` inside
`<dc:title>`; the `x-default` language item wins if present,
otherwise the first item.

## FS-refresh check

Inside `get_image`, before returning, we call:

```rust
fn refresh_needed_from_fs(image_path: &Path, cached: &ImageDetails) -> bool {
    let last_read = cached.meta_read_at.unwrap_or(0);
    let src_mtime = mtime_ms(image_path);
    let sidecar_mtime = mtime_ms(&sidecar_path_for(image_path));
    src_mtime > last_read || sidecar_mtime > last_read
}
```

If it returns `true`, we call `read_all`, `resync_user_meta_from_fs`
(which updates the DB row), and `set_meta_read_at_now` — then serve
the fresh state.

We still watch the legacy sidecar's mtime here because a user might
have run Lightroom against the same folder between two Magpie
scans, updating a `.xmp` without touching the source. Once Magpie
saves, the sidecar is cleaned up and the source-file mtime becomes
the sole trigger.

This is how an external tag edit (Windows Explorer) becomes visible
in Magpie on next click, without needing a full library rescan.
