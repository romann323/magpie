# File formats

Magpie's tag/title write path is powered by a small trait-based
plug-in system rather than a hard-coded `match ext { … }`. This page
describes what already exists.

## The `FormatHandler` trait

Every supported file type is represented by a Rust type that
implements the trait declared in `src-tauri/src/core/formats/mod.rs`:

```rust
pub trait FormatHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn kind(&self) -> FormatKind;
    fn can_write_tags(&self) -> bool;

    fn read_technical(&self, path: &Path) -> TechnicalMeta;
    fn read_user(&self, path: &Path) -> AppResult<UserMeta>;
    fn write_user(&self, path: &Path, meta: &UserMeta) -> AppResult<()>;
}
```

- `name` — human-readable label shown in the details panel
  (e.g. `"JPEG (XMP APP1)"`).
- `extensions` — lowercase extensions this handler owns
  (`&["jpg", "jpeg"]`).
- `kind` — image / video / document / other. Used to classify
  entries and skip inappropriate operations (thumbnails, previews).
- `can_write_tags` — advisory. If `false`, the frontend disables
  the editable fields and shows a warning; the backend still returns
  a clean error from `write_user` if it's called anyway.
- `read_technical` — the read-only metadata shown at the bottom of
  the details panel. An ordered list of `(label, value)` pairs.
- `read_user` — the editable title / tags surface.
- `write_user` — atomically writes user meta into the source file.

## The `FormatRegistry`

`FormatRegistry` maps each lowercase extension to exactly one
handler. It's constructed once at startup, wrapped in `Arc`, and
carried by `AppServices`.

Handlers register themselves in `FormatRegistry::new()`. The registry
is the sole authority for:

- **Which extensions are scannable.** `scanner` calls
  `registry.for_ext(ext).is_some()` to decide whether to index a
  file.
- **Which extensions can be edited.** `meta_write::write_metadata_to_source`
  looks up the handler and calls `write_user`; if the handler
  returns a `WriteNotSupported` error the write is refused cleanly
  (no fallback to sidecars).
- **Where technical/user metadata comes from.** `meta_read::read_all`
  builds an `ImageMetaFromFile` by calling
  `handler.read_technical(path)` and `handler.read_user(path)`,
  merged with any legacy sidecar values.

## Writing tags: two paths

`FormatHandler::can_write_tags` reports only what the *native*
handler can do. On Windows, `meta_write::write_metadata_to_source`
also consults `core::formats::win_shell`, which wraps
`SHGetPropertyStoreFromParsingName` + `IPropertyStore` — the same
COM interface behind Explorer's *Properties → Details* dialog.

| Path | When it's used | What it writes |
| ---- | -------------- | -------------- |
| Native handler | `handler.can_write_tags() == true` | The full XMP packet inside the container. Preserves foreign fields (`xmp:Rating`, `dc:description`). |
| Windows Shell fallback | Otherwise, on Windows | `System.Title` and `System.Keywords` via `IPropertyStore`. Any other properties (rating, author, GPS) are left as they are. |

The IPC-facing `ImageDetails.canWriteTags` is the OR of the two,
and its per-extension answer for the Shell fallback is cached on
`AppServices::shell_write_cache`. On non-Windows platforms
`win_shell` compiles to a no-op that reports "not available".

## Handler catalogue

The "Native writes?" column is what
`FormatHandler::can_write_tags` returns. The "Shell fallback?"
column is what a stock Windows 10 / 11 install typically offers via
`IPropertyStore`; it depends on which property handlers / codec
packs the user has installed.

| Handler | Kind | Extensions | Native writes? | Shell fallback? | Notes |
| ------- | ---- | ---------- | -------------- | --------------- | ----- |
| JPEG | Image | `.jpg`, `.jpeg` | ✅ | not needed | XMP APP1 segment. |
| PNG | Image | `.png` | ✅ | not needed | `iTXt` chunk with `XML:com.adobe.xmp`. |
| WebP | Image | `.webp` | ✅ | not needed | `XMP ` RIFF chunk. Converts simple-form VP8L to extended VP8X on write when needed. |
| GIF | Image | `.gif` | ✅ (GIF89a only) | not needed | Adobe's XMP-in-GIF Application Extension. |
| TIFF | Image | `.tif`, `.tiff`, `.dng` | ❌ | ✅ | Reads tag 700 XMP + EXIF natively; write goes through the Shell. |
| HEIC / HEIF / AVIF | Image | `.heic`, `.heif`, `.avif` | ❌ | ✅ | Native read of box-level EXIF; Shell writes. |
| JPEG XL | Image | `.jxl` | ❌ | ⚠️ handler-dependent | Windows JXL codec pack ships the property handler; if not installed, library-only. |
| JPEG 2000 | Image | `.jp2`, `.j2k`, `.jpx` | ❌ | ⚠️ | Same as JXL. |
| PSD | Image | `.psd`, `.psb` | ❌ | ✅ | Photoshop ships a property handler; Windows also ships one for basic keywords. |
| PDF | Document | `.pdf` | ❌ | ⚠️ | Requires an installed PDF property handler (Adobe Reader, PDF-XChange, …). |
| MP4/MOV | Video | `.mp4`, `.m4v`, `.mov` | ❌ | ✅ | Native duration/resolution read; Shell writes tags into `moov/udta` boxes. |
| Matroska / WebM | Video | `.mkv`, `.webm` | ❌ | ⚠️ | Shell fallback works if the Matroska property handler is installed (VideoLAN / K-Lite). |
| AVI | Video | `.avi` | ❌ | ✅ | |
| WMV/ASF | Video | `.wmv`, `.asf` | ❌ | ✅ | |
| MPEG-TS | Video | `.mpg`, `.mpeg`, `.mts`, `.m2ts` | ❌ | ⚠️ | Container-only native read. |
| 3GP | Video | `.3gp`, `.3g2` | ❌ | ✅ | |
| RAW variants | Image | `.cr2`, `.cr3`, `.nef`, `.arw`, `.raf`, `.orf`, `.rw2`, `.pef`, `.srw`, `.x3f` | ❌ | ⚠️ vendor codec pack | Native EXIF read; Shell writes when the vendor's Windows codec pack is present. Sigma X3F: Sigma Photo Pro / Windows RAW image extension. |
| BMP | Image | `.bmp`, `.dib` | ❌ | ✅ | Windows ships a property handler. |
| OpenEXR | Image | `.exr` | ❌ | ❌ | No standard Windows property handler ships for `.exr`; library-only. |
| Radiance HDR | Image | `.hdr`, `.pic` | ❌ | ❌ | Library-only. |
| SVG | Image | `.svg`, `.svgz` | ❌ | ❌ | Library-only. |

Legend: ✅ writes today, ❌ never writes on the format, ⚠️ depends
on which property handler the user has installed on their machine.

## Where the code lives

```
src-tauri/src/core/formats/
├── mod.rs                # FormatHandler trait + FormatRegistry + TechnicalMeta / UserMeta
├── common.rs             # Shared atomic_write, EXIF → TechnicalMeta, dimensions
├── xmp_packet.rs         # XMP parse / build (single source of truth for XMP wire format)
├── win_shell.rs          # IPropertyStore fallback (Windows only; stubbed elsewhere)
├── jpeg.rs               # writable natively
├── png.rs                # writable natively
├── webp.rs               # writable natively
├── gif.rs                # writable natively (GIF89a only)
├── tiff.rs               # native read, Shell write
└── stubs.rs              # all the other read-natively handlers, one struct per family
```

`stubs.rs` deliberately groups similar formats so adding a new
"native read, Shell write" format is one struct + one line in
`FormatRegistry::new`.
