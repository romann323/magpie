# Metadata write path

## Entry point

```rust
pub fn write_metadata_to_source(
    registry: &FormatRegistry,
    path: &Path,
    patch_title:    Option<Option<String>>,
    patch_subjects: Option<Vec<String>>,
) -> AppResult<()>;
```

Called from `apply_patch_and_persist` in `commands/images.rs`, which
is itself called by both `update_image_metadata` and
`batch_update_metadata`.

**Policy.** All user metadata Magpie edits is embedded directly
inside the source file. Magpie never creates `.xmp` sidecar files.
The dispatcher picks one of two write paths:

1. **Native handler** — for formats where Magpie can rewrite the
   container itself (JPEG, PNG, WebP, GIF89a today).
2. **Windows Shell fallback** (`core::formats::win_shell`) — for
   every other registered format on Windows. This uses
   `IPropertyStore` (`SHGetPropertyStoreFromParsingName` +
   `SetValue` + `Commit`), the same COM interface behind Explorer's
   *Properties → Details* dialog. Anything a user could tag by hand
   in Explorer is writable by Magpie via this path, and vice-versa.

## Format dispatch

```rust
let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
let handler = registry.for_ext(ext).ok_or(AppError::UnsupportedFormat)?;

let existing = if handler.can_write_tags() {
    handler.read_user(path).unwrap_or_default()
} else {
    win_shell::read_user_meta(path).unwrap_or_default()
};
let edits = merge(existing, patch_title, patch_subjects);

if handler.can_write_tags() {
    handler.write_user(path, &edits)?;                 // native
} else {
    win_shell::write_user_meta(path, &edits)?;         // Windows Shell
}
```

The registry decides which native handler owns a given extension.
`can_write_tags()` on the handler decides whether we go native or
fall back to the Shell. See [File formats](./file-formats.md) for
the handler catalogue.

The IPC-facing `ImageDetails.writeMode` sent to the frontend is
computed with the same priority the writer uses at dispatch time:

```rust
let write_mode = if native_can_write {
    WriteMode::Native            // JPEG / PNG / WebP / GIF89a
} else if services.shell_can_write_tags(path) {
    WriteMode::Shell             // RAW / HEIC / TIFF / MP4 / MOV / ...
} else {
    WriteMode::LibraryOnly       // BMP / DIB / SVG / EXR / HDR / ...
};
let can_write_tags =
    !matches!(write_mode, WriteMode::LibraryOnly);
```

`shell_can_write_tags` is a per-extension cache on top of
`win_shell::can_write_tags(path)`, which performs a **two-stage**
probe:

1. Open the store with `SHGetPropertyStoreFromParsingName(...,
   GPS_READWRITE, IPropertyStore)`. Extensions with no writable
   property handler (`STG_E_ACCESSDENIED` / `REGDB_E_CLASSNOTREG`)
   fail out here — SVG, EXR, HDR, PDF-without-Adobe, WebM, MKV, …
2. Ask the store *per-property* whether it accepts writes to
   `System.Keywords` via
   `IPropertyStoreCapabilities::IsPropertyWritable(&PKEY_Keywords)`.
   This step is what filters out BMP, DIB, GIF-shell, and WebP-shell
   — their handlers *open* a R/W store but reject `SetValue`
   on Keywords. Without this check the UI would happily enable
   editing and then silently drop the user's tags.

The frontend uses `writeMode` (not just `canWriteTags`) to show one
of three hint strings under the tag editor: *"embedded directly …
(XMP)"*, *"saved via the same Windows property system as Explorer's
Details tab"*, or *"stored in Magpie's library only"*.

Handlers with native `can_write_tags() == true`:

| Handler | Container | Notes |
| ------- | --------- | ----- |
| JPEG (`.jpg`, `.jpeg`) | APP1 XMP segment | Drops any existing standard-XMP or ExtendedXMP APP1 to avoid stacking. |
| PNG (`.png`) | `iTXt` chunk with keyword `XML:com.adobe.xmp` | Emitted immediately after IHDR; CRC recomputed. |
| WebP (`.webp`) | RIFF `XMP ` chunk | Upgrades simple-form VP8L to extended VP8X on write if needed. |
| GIF89a (`.gif`) | Application Extension block with Adobe's XMP magic trailer | Rejects GIF87a with a clear error. |

Every other registered extension (RAW families, MP4/MOV, MKV, WMV,
PDF, HEIC/AVIF, JPEG XL, TIFF/DNG, …) is served by the Windows Shell
fallback. Whether that fallback actually succeeds is a runtime
property of the machine: if `SHGetPropertyStoreFromParsingName(...,
GPS_READWRITE, IPropertyStore)` returns a live store for the file,
Magpie writes through it; otherwise it surfaces the OS error
verbatim wrapped in an `AppError::MetadataWrite`.

On non-Windows platforms `win_shell::write_user_meta` is a stub that
always returns
`Err(MetadataWrite("Windows Shell property system is not available on this platform."))`
until an equivalent per-platform fallback ships.

## Algorithm inside a writable handler

1. **Read the whole file.** All target formats top out well below
   typical RAM; a 20 MB JPEG is negligible.
2. **Extract the existing XMP** using the format-specific walker
   (`extract_xmp`).
3. **Parse it** with `xmp_packet::parse_xmp` into an `XmpUserMeta`.
4. **Merge the incoming `UserMeta`** with
   `xmp_packet::merge_user_edits`. This preserves any foreign fields
   (`xmp:Rating`, `dc:description`, GPS, etc.) so a Lightroom user
   never loses their star ratings just because Magpie doesn't
   surface them.
5. **Rebuild the packet** with `xmp_packet::build_xmp_packet`.
6. **Rewrite the container** — dropping any prior XMP block and
   splicing the new one in at the spec-recommended location.
7. **Atomic write** via `common::atomic_write_bytes`.

## Atomic file replace

Both writers funnel through `common::atomic_write_bytes`:

```rust
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let tmp = path.with_file_name(
        format!("{}.{}", path.file_name().unwrap().to_string_lossy(), WRITE_TMP_SUFFIX)
    );
    { let mut f = File::create(&tmp)?; f.write_all(bytes)?; f.sync_all().ok(); }
    fs::rename(&tmp, path).or_else(|e| { let _ = fs::remove_file(&tmp); Err(e) })?;
    Ok(())
}
```

Windows semantics: `fs::rename` uses `MoveFileExW` internally with
`MOVEFILE_REPLACE_EXISTING`, so the atomic swap works even when the
target exists. On crash mid-write, the original file is still
intact; at worst a temp file is left behind and is overwritten on
the next successful write.

## XMP packet builder

`xmp_packet::build_xmp_packet(&XmpUserMeta) -> String` emits a
minimal, spec-conformant XMP packet:

```xml
<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Magpie">
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

    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
```

Empty fields are omitted. Every text value is XML-escaped before
insertion so ampersands, angle brackets, and quotes never break the
packet. `xmp:Rating` and `dc:description` only appear if the file
already had them — Magpie's UI doesn't produce those fields but does
not clobber them.

## Legacy sidecar cleanup

After a successful embed, if `sidecar_path_for(image_path)` exists,
`write_metadata_to_source` removes it. The deletion is best-effort:
failure is logged but doesn't fail the save, because the source
file already holds the authoritative data.

## Failure modes and recovery

| Failure                                    | Effect                                                            |
| ------------------------------------------ | ----------------------------------------------------------------- |
| Unsupported extension (RAW, HEIC, TIFF, …) | Handler's `write_user` returns `Err(_)` before any I/O.           |
| DB transaction fails                       | Nothing is written to disk; caller receives `Err(_)`.             |
| Source file read fails                     | Return `Err(_)`. DB update from earlier is kept (retryable).      |
| Source file rewrite fails (tmp / rename)   | Temp file removed if possible; original intact; caller sees `Err`.|
| Embed succeeds; sidecar delete fails       | Save reports `Ok(())`; warning is logged; stale sidecar will be   |
|                                            | overridden by fresh embedded packet on next read.                 |
| Crash mid-write (any step)                 | Original file intact (atomic rename); temp file leaks and is      |
|                                            | eventually cleaned up on next successful write of same file.      |

## Test coverage

Integration tests in `src-tauri/tests/metadata_fs.rs`:

- `read_sidecar_end_to_end` — legacy sidecar read.
- `read_sidecar_case_variants` — namespace/case robustness.
- `fts_delete_after_tag_update_works` — FTS5 regression.
- `batch_tag_add_persists_for_every_image` — batch semantics.
- `embed_xmp_roundtrip_{jpeg,png,webp,gif}` — one roundtrip per
  writable handler.
- `write_never_creates_sidecar_for_jpeg` — no `.xmp` fallback.
- `write_removes_legacy_sidecar_after_embed` — legacy sidecar
  cleanup.
- `write_errors_or_uses_shell_for_stub_format` — `.cr2` either
  writes through the Shell fallback or errors cleanly; never a
  sidecar.
- `write_preserves_foreign_rating_and_description` — rating and
  description written by other tools survive Magpie's edits.
- `registry_recognises_every_expected_extension` — every advertised
  handler is registered.

Unit tests in `core::formats::xmp_packet::tests`:

- `roundtrip_preserves_all_fields`
- `windows_explorer_tags_read`
- `microsoft_only_keywords_read`
- `merge_preserves_foreign_rating_and_description`
