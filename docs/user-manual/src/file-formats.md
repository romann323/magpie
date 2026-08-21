# Supported file formats

Magpie can put every kind of file below into your library and let
you tag and search it. The difference between the categories is
whether Magpie can **save your tags into the file itself** or just
**remember them in the library**.

The upside of saving tags into the file is that they follow the file
around — copy it to another computer, upload it to a cloud folder,
send it to a friend, and the tags come along. The downside is that
Magpie has to know how to write into the file's format, and some
formats are trickier than others.

## Files where Magpie can save tags into the file

Magpie writes titles and tags using **two** mechanisms, whichever
fits the file best:

- For a small set of image formats Magpie writes the tags itself,
  using the industry-standard **XMP** metadata packet that Adobe
  Lightroom, digiKam, and Windows Explorer all understand.
- For everything else, Magpie asks **Windows** to do it, using the
  exact same code path as the *Properties → Details* dialog you can
  open by right-clicking a file in Explorer.

That means: **if Windows Explorer can set tags on a file, Magpie
can too.**

### Magpie writes directly (XMP)

These four containers Magpie writes into by itself, using the
industry-standard **XMP** slot. Works on any OS, regardless of what
Windows codec packs happen to be installed.

| Format | Extensions | Notes |
| ------ | ---------- | ----- |
| **JPEG** | `.jpg`, `.jpeg`, `.jpe`, `.jfif` | The most common photo format. |
| **PNG** | `.png` | Great for screenshots and lossless images. |
| **WebP** | `.webp` | Modern web-friendly format. |
| **GIF** | `.gif` | GIF89a only; old GIF87a files can't hold tags. |

### Windows writes for us (Shell property system)

For every other format below Magpie asks Windows to save the tags,
using the exact same code path Explorer's *Properties → Details*
dialog uses. Verified working on a stock Windows 11 install:

- **Camera RAW** — `.cr2`, `.cr3`, `.crw` (Canon), `.nef`, `.nrw`
  (Nikon), `.arw`, `.sr2`, `.srf` (Sony), `.raf` (Fuji), `.orf`,
  `.ori` (Olympus), `.rw2`, `.rwl` (Panasonic), `.pef` (Pentax),
  `.srw` (Samsung), `.dng` (Adobe/generic), `.x3f` (Sigma).
- **Modern HEIF family** — `.heic`, `.heif`, `.hif`, `.avif`.
- **TIFF family** — `.tif`, `.tiff`.
- **JPEG XL / JPEG XR / HD Photo** — `.jxl`, `.jxr`, `.wdp`.
- **Video / MP4 family** — `.mp4`, `.m4v`, `.mov`, `.3gp`, `.3gpp`,
  `.3g2`.
- **Legacy Windows video / audio** — `.wmv`, `.asf`.

If the tag editor is enabled and you don't see the amber warning
below the tag box, Windows agreed to save into the file. You can
double-check by right-clicking the file in Explorer and opening
*Properties → Details*.

## Files Magpie remembers but doesn't tag inside

The following formats have no writable property handler on a stock
Windows 11 install — Magpie still lets you tag them, but the tags
live in the Magpie library only:

- **Raster with no keyword support** — `.bmp`, `.dib`, `.svg`,
  `.exr`, `.hdr`, `.hdp`.
- **JPEG 2000 family** — `.jp2`, `.j2k`, `.j2c`, `.jpx`, `.jif`.
- **Photoshop** — `.psd`, `.psb` *(no keyword-writable handler
  registered by Windows; Photoshop itself does write, but its
  handler doesn't expose `System.Keywords`).*
- **PDF** — `.pdf` *(depends on the installed reader — Adobe
  Reader DC registers a writable handler, most other viewers
  don't).*
- **MPEG transport / Matroska / MP2** — `.ts`, `.mts`, `.m2ts`,
  `.mkv`, `.mks`, `.mka`, `.webm`, `.avi`, `.qt`.
- **Sony pixel-shift RAW** — `.arq`.

For these Magpie stores tags **only in its library index**. Search
and filtering still work; the tag just won't follow the file if you
copy it elsewhere. The tag editor shows a short amber note whenever
this happens.

> The list above reflects a stock Windows 11 24H2 install. If you
> install a third-party codec pack (e.g. Adobe DNG Codec, Panasonic
> RAW codec, PDF-XChange), Magpie automatically starts writing into
> those formats — no configuration required. Restart Magpie once
> after installing a new codec so the capability cache refreshes.

## Files Magpie ignores

Anything not in the tables above is skipped when you scan a folder.
That's intentional — a system file or ZIP archive doesn't usually
belong in a photo/video library.

If you want a new file type supported, that's a normal thing to
ask for. The Magpie backend has a small "format handler" plug-in
system so new types can be added without touching the rest of the
app.
