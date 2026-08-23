# Supported file formats

Magpie can put every file below into your library and let you tag
and search it. **Every recognised file is fully taggable** — the tags
just don't go into the file itself. They live in Magpie's own
database in your Windows AppData folder; see
[Where your data lives](./data-storage.md) for the details.

That means: **if Magpie shows the file in the grid, you can tag it.**
No more "sorry, this format has no writable property handler" warning.

## Images

- **JPEG** — `.jpg`, `.jpeg`, `.jpe`, `.jfif`
- **PNG** — `.png`
- **WebP** — `.webp`
- **GIF** — `.gif`
- **TIFF** — `.tif`, `.tiff`
- **HEIF family** — `.heic`, `.heif`, `.hif`, `.avif`
- **JPEG XL / JPEG XR / HD Photo** — `.jxl`, `.jxr`, `.wdp`
- **Older raster formats** — `.bmp`, `.dib`, `.exr`, `.hdr`, `.hdp`
- **JPEG 2000 family** — `.jp2`, `.j2k`, `.j2c`, `.jpx`, `.jif`
- **Vector** — `.svg`
- **Photoshop** — `.psd`, `.psb`

## Camera RAW

- **Canon** — `.cr2`, `.cr3`, `.crw`
- **Nikon** — `.nef`, `.nrw`
- **Sony** — `.arw`, `.sr2`, `.srf`, `.arq`
- **Fuji** — `.raf`
- **Olympus** — `.orf`, `.ori`
- **Panasonic** — `.rw2`, `.rwl`
- **Pentax** — `.pef`
- **Samsung** — `.srw`
- **Sigma** — `.x3f`
- **Adobe / generic** — `.dng`

## Video

- **MP4 family** — `.mp4`, `.m4v`, `.mov`, `.3gp`, `.3gpp`, `.3g2`
- **Legacy Windows** — `.wmv`, `.asf`
- **MPEG transport / Matroska / AVI** — `.ts`, `.mts`, `.m2ts`,
  `.mkv`, `.mks`, `.mka`, `.webm`, `.avi`, `.qt`

## Documents

- **PDF** — `.pdf`

## Existing tags in your files

If a JPEG (or any format above) already carries XMP tags from Adobe
Bridge / Lightroom, or the Explorer *Properties → Details* dialog,
Magpie **reads them once** on the first scan and imports them into
the folder's database. From that point on you can edit the tags in
Magpie freely — but Magpie will never write into the source file
again. See [Interoperability with other tools](./interop.md).

## Files Magpie ignores

Anything not in the tables above is skipped when you scan a folder.
That's intentional — a system file or ZIP archive doesn't usually
belong in a photo/video library.

If you want a new file type supported, that's a normal thing to
ask for. The Magpie backend has a small "format handler" plug-in
system so new types can be added without touching the rest of the
app.
