# File formats

Magpie's metadata read path is powered by a small trait-based
plug-in system rather than a hard-coded `match ext { … }`. Every
handler is **read-only** — see
[Database redesign](./db-redesign.md) and
[Adding a format handler](./adding-a-format-handler.md).

## The `FormatHandler` trait

Every supported file type is represented by a Rust type that
implements the trait declared in `src-tauri/src/core/formats/mod.rs`:

```rust
pub trait FormatHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn kind(&self) -> FormatKind;

    fn read_technical(&self, path: &Path) -> TechnicalMeta;
    fn read_user(&self, path: &Path) -> AppResult<UserMeta>;
}
```

- `name` — human-readable label shown in the details panel
  (e.g. `"JPEG"`).
- `extensions` — lowercase extensions this handler owns
  (`&["jpg", "jpeg"]`).
- `kind` — image / video / document / other. Used to classify
  entries and skip inappropriate operations (thumbnails, previews).
- `read_technical` — the read-only metadata shown at the bottom of
  the details panel. An ordered list of `(label, value)` pairs.
- `read_user` — extract title + tags from the file (XMP, EXIF,
  container-specific atoms). Called on import.

## The `FormatRegistry`

`FormatRegistry` maps each lowercase extension to exactly one
handler. It's constructed once at startup, wrapped in `Arc`, and
carried by `AppServices`.

Handlers register themselves in `FormatRegistry::new()`. The registry
is the sole authority for:

- **Which extensions are scannable.** `scanner` calls
  `registry.for_ext(ext).is_some()` to decide whether to index a
  file.
- **Where technical/user metadata comes from.**
  `metadata::read::read_all` builds an `ImageMetaFromFile` by
  calling `handler.read_technical(path)` and
  `handler.read_user(path)`, then merging in the Windows Shell
  property store result and any legacy `.xmp` sidecar.

## First-scan tag import

On the *first* time the scanner sees a file, `read_user` and the
Windows Shell fallback are consulted so tags that a previous
tagger (Lightroom, Adobe Bridge, Explorer's *Properties → Details*)
already put in the file are imported into the folder's
`library.db`. After that, the DB is the source of truth and the
file is never read again for user metadata unless its `mtime`
moves forward.

## Handler catalogue

The "Reads tags" column describes what the native handler can pull
out during a first-scan import. Formats whose native handler can't
parse tags still get scanned into the DB — the Windows Shell
fallback usually fills in title/keywords on Windows.

| Handler | Kind | Extensions | Reads tags? | Notes |
| ------- | ---- | ---------- | ----------- | ----- |
| JPEG | Image | `.jpg`, `.jpeg` | ✅ XMP APP1 | |
| PNG | Image | `.png` | ✅ iTXt XMP | |
| WebP | Image | `.webp` | ✅ RIFF XMP | |
| GIF | Image | `.gif` | ✅ (GIF89a) | Adobe XMP-in-GIF Application Extension |
| TIFF | Image | `.tif`, `.tiff` | ✅ tag 700 | |
| HEIC / HEIF / AVIF | Image | `.heic`, `.heif`, `.hif`, `.avif` | ❌ (Shell import) | Native read of box-level EXIF for `taken_at` only |
| JPEG XL | Image | `.jxl` | ❌ (Shell import) | |
| JPEG XR / HD Photo | Image | `.jxr`, `.wdp` | ❌ (Shell import) | |
| JPEG 2000 | Image | `.jp2`, `.j2k`, `.j2c`, `.jpx`, `.jif` | ❌ (Shell import) | |
| PSD | Image | `.psd`, `.psb` | ❌ (Shell import) | |
| PDF | Document | `.pdf` | ❌ (Shell import) | |
| MP4/MOV family | Video | `.mp4`, `.m4v`, `.mov`, `.3gp`, `.3gpp`, `.3g2` | ❌ (Shell import) | |
| Matroska / WebM | Video | `.mkv`, `.mks`, `.mka`, `.webm` | ❌ (Shell import) | |
| AVI | Video | `.avi` | ❌ (Shell import) | |
| WMV/ASF | Video | `.wmv`, `.asf` | ❌ (Shell import) | |
| MPEG-TS | Video | `.ts`, `.mts`, `.m2ts` | ❌ (Shell import) | |
| Canon RAW | Image | `.cr2`, `.cr3`, `.crw` | ❌ (Shell import) | |
| Nikon RAW | Image | `.nef`, `.nrw` | ❌ (Shell import) | |
| Sony RAW | Image | `.arw`, `.sr2`, `.srf`, `.arq` | ❌ (Shell import) | |
| Fuji RAW | Image | `.raf` | ❌ (Shell import) | |
| Olympus RAW | Image | `.orf`, `.ori` | ❌ (Shell import) | |
| Panasonic RAW | Image | `.rw2`, `.rwl` | ❌ (Shell import) | |
| Pentax RAW | Image | `.pef` | ❌ (Shell import) | |
| Samsung RAW | Image | `.srw` | ❌ (Shell import) | |
| Sigma RAW | Image | `.x3f` | ❌ (Shell import) | |
| Adobe/generic DNG | Image | `.dng` | ✅ (TIFF-family) | |
| BMP | Image | `.bmp`, `.dib` | ❌ (Shell import) | |
| OpenEXR | Image | `.exr` | ❌ | Library-only tags after import |
| Radiance HDR | Image | `.hdr`, `.hdp` | ❌ | Library-only tags after import |
| SVG | Image | `.svg` | ❌ | Library-only tags after import |

"Shell import" means: the format's native `read_user` returns
empty, and `metadata::read::read_all` then asks Windows'
`SHGetPropertyStoreFromParsingName` for `System.Title` +
`System.Keywords`. So on Windows the tag import still works for
any format Explorer's *Details* pane recognises. On other
platforms (future) these formats will simply start empty.

## Where the code lives

```
src-tauri/src/core/formats/
├── mod.rs           # FormatHandler trait + FormatRegistry + TechnicalMeta / UserMeta
├── common.rs        # Shared: EXIF → TechnicalMeta, dimensions, verbatim-prefix helpers
├── xmp_packet.rs    # XMP parser (read-only)
├── win_shell.rs     # IPropertyStore fallback reader (Windows only; stub elsewhere)
├── jpeg.rs          # native XMP APP1 reader
├── png.rs           # native iTXt XMP reader
├── webp.rs          # native RIFF XMP reader
├── gif.rs           # native Application-Extension XMP reader (GIF89a)
├── tiff.rs          # native tag 700 XMP reader
└── stubs.rs         # everything else, one struct per family
```

`stubs.rs` deliberately groups similar formats so adding a new
"read technical + Shell-import user meta" format is one struct +
one line in `FormatRegistry::new`.
