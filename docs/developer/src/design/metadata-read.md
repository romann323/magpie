# Metadata read path

## Entry point

```rust
pub fn read_all(registry: &FormatRegistry, path: &Path) -> AppResult<ImageMetaFromFile>;
```

Called from:

- The scanner (initial scan and rescans), to populate the
  `images` row in `magpie.db`.
- `get_image` (Tauri command), when the source file's `mtime` has
  moved forward since the row's cached `mtime_ms`.

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

`ImageMetaFromFile` is used **only on import** (first scan of a
file, or on mtime bump). After the row exists in the DB, subsequent
edits stay in the DB via `queries::apply_metadata_patch`.

## Stages

### 1. Ask the handler

```rust
let handler = registry.for_ext(ext).ok_or(AppError::UnsupportedFormat)?;
let user      = handler.read_user(path)?;
let technical = handler.read_technical(path);
```

`read_user` returns the editable surface (title + tags).
`read_technical` returns the ordered `TechnicalMeta` list used by
the UI and by the scanner for dimensions / EXIF-derived fields
(camera, taken_at). See [File formats](./file-formats.md).

Each handler decides how to fetch this. Native handlers (JPEG, PNG,
WebP, GIF, TIFF) use the shared `xmp_packet::parse_xmp` helper;
stub handlers (HEIF, video, PDF, RAW, basic raster) implement
`read_technical` from magic bytes and defer `read_user` to the
Windows Shell fallback.

### 2. Windows Shell property store (import-only, Windows-only)

On Windows, if `handler.read_user` didn't return a title or tags
(typical for RAW, HEIC, MP4, PDF), `win_shell::read_user_meta` is
called. It uses `SHGetPropertyStoreFromParsingName` to open the
file's shell property store read-only and asks for `System.Title`
+ `System.Keywords`. This is how tags that a user typed into
Explorer's *Properties → Details* dialog get imported into Magpie
on first scan.

`win_shell` is strictly read-only after the redesign — there is no
matching write path.

### 3. Legacy sidecar XMP

```rust
let sidecar = read_sidecar(&sidecar_path_for(path)).ok();
```

Sidecar path is `<file basename>.xmp`. Sidecars are a **legacy
compatibility** path: Magpie no longer produces them, but older
Magpie installs and other tools (Lightroom, digiKam) did. If a
sidecar file exists it's parsed with the same XMP parser used for
embedded packets and its fields are unioned into the result.

### 4. Merge and fold into DB row

`apply_user_meta` folds the handler's `UserMeta`, then the Windows
Shell result, then the legacy sidecar (in that order — sidecar
wins if it exists, matching Lightroom conventions on read). The
merged `ImageMetaFromFile` is then written to `magpie.db` via
`queries::set_image_meta`.

## XMP parser

`xmp_packet::parse_xmp(bytes) -> XmpUserMeta` is a hand-written
streaming parser built on `quick_xml`. It normalises
casing/namespaces because tools in the wild are notoriously
inconsistent (`<X:XMPMETA>`, `<x:xmpmeta>`, and mixed-case in the
same file).

Recognises both `dc:subject` (standard) and
`MicrosoftPhoto:LastKeywordXMP` (Windows Explorer); unions the two
sets, deduping case-insensitively while preserving the first
observed casing. Handles both `<rdf:Alt>` and `<rdf:Seq>` inside
`<dc:title>`; the `x-default` language item wins if present,
otherwise the first item.

## FS-refresh check on `get_image`

Inside the `get_image` command, before returning:

```rust
if let Ok(fs_meta) = std::fs::metadata(&abs) {
    let disk_mtime = fs_meta.modified()?.duration_since(UNIX_EPOCH)?.as_millis() as i64;
    if disk_mtime > row.mtime_ms {
        let fresh = read_all(&registry, &abs)?;
        queries::set_image_meta(&mut conn, id, &fresh)?;
    }
}
```

Simple mtime comparison. If the file was modified after import,
re-read metadata and overwrite the row. This is how an external
tag edit (Windows Explorer, Lightroom export) becomes visible in
Magpie on next click, without needing a full library rescan.

**Note:** the mtime bump wipes Magpie-side edits with whatever's
currently in the file. In the new model, once the file is in the
DB, edits should happen in Magpie. If you edit tags in Explorer
after Magpie has been in charge, those Explorer edits win the
next time `get_image` is called for that file — because the file's
mtime is now newer, so Magpie re-reads it. Users who want to switch
tagging tools mid-flight should be aware of this trade-off.
