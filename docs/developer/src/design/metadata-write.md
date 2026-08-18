# Metadata write path

## Entry point

```rust
pub fn merge_and_write_sidecar(
    image_path: &Path,
    patch_title:       Option<Option<String>>,
    patch_description: Option<Option<String>>,
    patch_rating:      Option<Option<i64>>,
    patch_subjects:    Option<Vec<String>>,
) -> PicOrgResult<()>;
```

Called from `apply_patch_and_write_sidecar` in
`commands/images.rs`, which is itself called by both
`update_image_metadata` and `batch_update_metadata`.

## Algorithm

1. **Read existing.** `meta_read::read_all(image_path)?` returns the
   current on-disk state (or `None` on error — we treat that as an
   empty baseline).
2. **Apply patch.** For each of the four fields, if the patch has
   `Some(_)`, overwrite; otherwise keep the current value.
3. **Build packet.** `build_xmp_packet(&UserMetadata)` produces a
   full XMP packet as a `String`.
4. **Write sidecar.** `write_sidecar(image_path, &meta)` writes to
   `<image>.xmp` via atomic temp + rename.
5. **Embed in source.** `embed_xmp_in_source(image_path, packet
   .as_bytes())` injects into JPEGs; returns `Ok(false)` for other
   formats.
6. **Log outcome.**

Step 5 can fail (read-only source, unusual JPEG, disk full).
We treat this as a warning, not an error — the sidecar (step 4) is
authoritative and always succeeds. This matches Lightroom's behaviour
of "sidecar is the truth if it's newer".

## Sidecar writer

```rust
pub fn write_sidecar(image_path: &Path, meta: &UserMetadata) -> PicOrgResult<()> {
    let sidecar = sidecar_path_for(image_path);   // Photo.jpg → Photo.xmp
    let tmp     = sidecar.with_extension("xmp.tmp");
    let xml     = build_xmp_packet(meta);

    { // write body
        let mut f = File::create(&tmp)?;
        f.write_all(xml.as_bytes())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &sidecar)?;                  // atomic on same volume
    Ok(())
}
```

Windows semantics: `fs::rename` uses `MoveFileExW` internally with
`MOVEFILE_REPLACE_EXISTING`, so the atomic swap works even when the
target exists.

## XMP packet builder

`build_xmp_packet(&UserMetadata) -> String` writes:

```xml
<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="PicOrg">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:xmp="http://ns.adobe.com/xap/1.0/"
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmp:Rating="4"
        xmp:MetadataDate="2026-08-17T22:38:11+03:00">

      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Sunset over Vík</rdf:li>
        </rdf:Alt>
      </dc:title>

      <dc:description>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Long exposure, ND1000.</rdf:li>
        </rdf:Alt>
      </dc:description>

      <dc:subject>
        <rdf:Bag>
          <rdf:li>Iceland</rdf:li>
          <rdf:li>sunset</rdf:li>
        </rdf:Bag>
      </dc:subject>

      <!-- Windows Explorer-compatible mirror of dc:subject -->
      <MicrosoftPhoto:LastKeywordXMP
          xmlns:MicrosoftPhoto="http://ns.microsoft.com/photo/1.0/">
        <rdf:Bag>
          <rdf:li>Iceland</rdf:li>
          <rdf:li>sunset</rdf:li>
        </rdf:Bag>
      </MicrosoftPhoto:LastKeywordXMP>

    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
```

Empty fields are omitted (e.g. no title → no `<dc:title>` element).
`xmp:MetadataDate` is set to the current time in RFC 3339.

Every text value is XML-escaped by `xml_escape(s)` before insertion
so ampersands, angle brackets, and quotes never break the packet.

## JPEG segment writer

```rust
pub fn embed_xmp_in_source(path: &Path, xmp_bytes: &[u8]) -> PicOrgResult<bool>;
fn   embed_xmp_in_jpeg(path: &Path, xmp_bytes: &[u8])     -> PicOrgResult<()>;
```

Algorithm:

1. Read the whole file (JPEGs are small enough — a 20 MB photo is
   negligible).
2. Validate `[0xFF, 0xD8]` (SOI).
3. Build a new APP1 segment:
   - `0xFF 0xE1` marker.
   - `u16` big-endian length (2 + marker + xmp).
   - `http://ns.adobe.com/xap/1.0/\0` header.
   - The XMP packet bytes verbatim.
4. Walk the file scanning marker segments:
   - Standalone markers (SOI, EOI, RSTn) are copied.
   - Length-prefixed segments (APP0, DQT, DHT, SOF, …) are copied
     verbatim **except** APP1 segments whose payload starts with
     the standard XMP marker or the ExtendedXMP marker — those are
     dropped.
   - On SOS (`0xFFDA`), copy the rest of the file (compressed
     image data + EOI) without further parsing.
5. Build the output as `[SOI][new XMP APP1][…other segments…][SOS + data]`.
6. Write atomically to `<original>.picorg-tmp` and rename over the
   source.

Adobe's XMP specification recommends the standard XMP APP1 be the
first APP1 after SOI, which is exactly where we put it.

### Why we don't use `img-parts` or similar

Two reasons:

1. No new dependency: keeps the crate graph small and the audit
   surface tight.
2. The logic is 60 lines and comprehensively tested; a general-
   purpose crate would be pulling in support we don't need
   (RIFF/WebP write, HEIF, PSD, …).

### ExtendedXMP

If the XMP packet is larger than 65533 bytes it can't fit in a
single APP1 segment. Adobe's ExtendedXMP mechanism splits it
across an "extended" segment with its own marker
(`http://ns.adobe.com/xmp/extension/\0`). PicOrg's writer errors
out in this case:

```rust
if payload_len > JPEG_MAX_SEGMENT_PAYLOAD {
    return Err(PicOrgError::MetadataWrite("XMP too large".into()));
}
```

In practice our packets are always under 4 KB. If we ever hit the
limit (e.g. a photo with 500 tags), the reader side already
supports ExtendedXMP; writing support would be a small extension.

## Failure modes and recovery

| Failure                                    | Effect                                                            |
| ------------------------------------------ | ----------------------------------------------------------------- |
| DB transaction fails                       | Nothing is written to disk; caller receives `Err(_)`.             |
| Sidecar temp write fails                   | Partial temp file removed; caller receives `Err(_)`.              |
| Sidecar rename fails                       | Temp file removed on retry; caller receives `Err(_)`.             |
| Sidecar succeeds; embed fails              | Sidecar contains the new state, embed does not.                   |
|                                            | Warning logged. Caller sees `Ok(())`.                             |
| Crash mid-write (any step)                 | Original file intact (atomic rename); temp file leaks and is      |
|                                            | eventually cleaned up on next successful write of same file.      |

## Test coverage

Integration tests in `src-tauri/tests/metadata_fs.rs`:

- `read_sidecar_end_to_end` — builds a sidecar and reads it back.
- `read_sidecar_case_variants` — namespace/case robustness.
- `fts_delete_after_tag_update_works` — regression for the FTS5 bug.
- `batch_tag_add_persists_for_every_image` — batch semantics.
- `embed_xmp_roundtrip_jpeg` — build a minimal JPEG, embed XMP,
  extract it back, verify content and JPEG validity.
