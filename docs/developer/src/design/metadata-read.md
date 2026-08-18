# Metadata read path

## Entry point

```rust
pub fn read_all(path: &Path) -> PicOrgResult<ImageMeta>;
```

Called from:

- The scanner (initial scan and rescans), to populate the DB.
- `get_image` (from a Tauri command), when a FS-refresh check
  detects that the sidecar / source has changed since the DB last
  saw it.

`ImageMeta` is the union of everything we can pull from disk:

```rust
pub struct ImageMeta {
    pub title:        Option<String>,
    pub comment:      Option<String>,   // dc:description
    pub rating:       Option<i64>,      // 0..=5
    pub tags:         Vec<String>,      // deduped, in insertion order
    pub taken_at:     Option<i64>,      // ms since Unix epoch
    pub camera_make:  Option<String>,
    pub camera_model: Option<String>,
    pub width:        Option<u32>,
    pub height:       Option<u32>,
}
```

## Stages

### 1. EXIF

```rust
let exif = exif::Reader::new()
    .continue_on_error(true)
    .read_from_container(&mut BufReader::new(File::open(path)?))?;
```

Fields extracted:

| EXIF tag                  | ImageMeta field   |
| ------------------------- | ----------------- |
| `DateTimeOriginal`        | `taken_at`        |
| `Make`                    | `camera_make`     |
| `Model`                   | `camera_model`    |
| `PixelXDimension` / EXIF or IFD dimensions | `width`  |
| `PixelYDimension`         | `height`          |

`taken_at` parsing handles both `YYYY:MM:DD HH:MM:SS` and ISO-8601
strings, and preserves the local time zone if `OffsetTimeOriginal`
is present.

If the file has no EXIF (PNG without an `eXIf` chunk, for
example), we fall back to `image::image_dimensions(path)?` for the
pixel size and leave the timing fields NULL.

### 2. Embedded XMP

```rust
let embedded = xmp::extract_embedded_xmp(path).ok().flatten();
```

For JPEGs this walks APP segments looking for
`http://ns.adobe.com/xap/1.0/\0` (see
[Metadata write path](./metadata-write.md#jpeg-segment-writer) for
the exact layout). Non-JPEG formats return `None` — v1's reader for
PNG/HEIC/TIFF embedded XMP is on the roadmap.

The read cap is 2 MiB — enough to catch a big Explorer-written XMP
with a thumbnail while still being cheap. Files smaller than that
are read in full anyway.

### 3. Sidecar XMP

```rust
let sidecar = read_sidecar(&sidecar_path_for(path)).ok();
```

Sidecar path is `<image basename>.xmp`. If the file exists it's
parsed with the same XMP parser used for embedded.

### 4. Merge

`apply_user_meta` is called first with the embedded XMP (if any),
then with the sidecar XMP (if any). The later call overwrites any
field the earlier call set. Result: **sidecar wins over embedded**.

Rationale: sidecar reflects the latest edit. Both Lightroom and
digiKam follow this convention.

## XMP parser

`parse_user_metadata(bytes) -> UserMetadata` is a hand-written
streaming parser built on `quick_xml`. Uses a simple state
machine:

```
Idle → SubjectBag → SubjectItem → SubjectBag → SubjectItem → …
     ↘ MsKeywordBag → MsKeywordItem → …
     ↘ Title → (li text captured) → Idle
     ↘ Description → (li text captured) → Idle
```

- Recognises both element form (`<xmp:Rating>4</xmp:Rating>`) and
  attribute form (`xmp:Rating="4"` on `<rdf:Description>`).
- Recognises both `dc:subject` (standard) and
  `MicrosoftPhoto:LastKeywordXMP` (Windows Explorer). Unions the two
  sets, deduping case-insensitively while preserving the first
  observed casing.
- Handles both `<rdf:Alt>` and `<rdf:Seq>` inside `<dc:title>` and
  `<dc:description>`; the `x-default` language item wins if present,
  otherwise the first item.

Everything is `<xmlns>` and `<case>` insensitive because tools in
the wild are notoriously inconsistent (I've seen `<X:XMPMETA>`,
`<x:xmpmeta>`, and mixed-case in the same file). See the
`read_sidecar_case_variants` test.

## FS-refresh check

Inside `get_image`, before returning, we call:

```rust
fn refresh_needed_from_fs(
    path: &Path,
    sidecar_mtime_ms: Option<i64>,
    meta_read_at: Option<i64>,
) -> bool {
    let last_read = meta_read_at.unwrap_or(0);
    let src_mtime_ms = fs::metadata(path).and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let sidecar_mtime_ms = sidecar_mtime_ms.unwrap_or(0);
    src_mtime_ms > last_read || sidecar_mtime_ms > last_read
}
```

If it returns `true`, we call `read_all`, `resync_user_meta_from_fs`
(which updates the DB row), and `set_meta_read_at_now` — then serve
the fresh state.

This is how an external tag edit (Windows Explorer) becomes visible
in PicOrg on next click, without needing a full library rescan.
