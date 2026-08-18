# Where your data lives

PicOrg is a **local, transparent** photo manager. Everything it stores
is either standard XMP (in your photos and next to them) or a cache
under your app data folder that you can delete at any time without
losing information.

## The three places PicOrg writes

### 1. Sidecar `.xmp` files (next to each photo)

For every photo you edit, PicOrg creates or updates a sibling `.xmp`
file:

```
📁 Trips\Iceland\
   IMG_2043.jpg      ← your photo, untouched (see next section)
   IMG_2043.xmp      ← Adobe-standard XMP sidecar written by PicOrg
```

The sidecar is a plain UTF-8 XML file you can open in any editor. It
follows the Lightroom convention (strip original extension, use
`.xmp`), so Lightroom, Bridge, digiKam, and darktable all read it.

### 2. Embedded XMP inside the source JPEG

**Since v1**, PicOrg also injects a standard XMP APP1 segment into the
source JPEG itself so tools that ignore sidecars (Windows Explorer's
Details tab, the Photos app, most viewers) still see your tags,
title, rating, and comment.

- Non-JPEG formats (PNG, HEIC, RAW, TIFF, WebP) get only the sidecar
  in v1. This is a safe default that never touches the pixels.
- The injection is atomic: PicOrg writes a temp file next to the
  original and renames over it, so a crash mid-write never leaves you
  with a truncated photo.
- If the source file is on read-only media, PicOrg logs a warning and
  falls back to sidecar-only.

### 3. PicOrg's own cache and index

Under `%APPDATA%\com.picorg.picorg\`:

```
📁 com.picorg.picorg\
   picorg.db                ← SQLite index of every photo you've added
   📁 thumbs\               ← WebP thumbnails, keyed by photo id
   📁 logs\
      picorg.log            ← rolling log; useful if something misbehaves
```

None of this is your data — it's derived from your files and can be
rebuilt at any time by rescanning. Feel free to delete the whole
folder to reset PicOrg.

## What PicOrg **never** does

- **It does not move or rename your photos.** Ever. Files stay where
  you put them.
- **It does not upload anything.** There is no server, no telemetry,
  no analytics.
- **It does not modify the pixels.** The only thing it changes in
  the source file is the XMP segment.
- **It does not touch photos in folders you haven't added.**

## Round-trip guarantees

If tool X wrote a tag into your photo, PicOrg reads it. If PicOrg
writes a tag, tool X sees it. Concretely:

| Tag written by            | Read back correctly in                                             |
| ------------------------- | ------------------------------------------------------------------ |
| Windows Explorer          | PicOrg, Lightroom, Bridge, digiKam                                 |
| PicOrg                    | Windows Explorer, Photos app, Lightroom, Bridge, digiKam           |
| Adobe Lightroom sidecar   | PicOrg (from sidecar), Bridge (from sidecar), digiKam              |
| digiKam                   | PicOrg (both embedded and sidecar), Lightroom, Bridge              |

## Portability

Your library is portable: copy the folder tree to another machine
(with the sidecar files) and every tag, rating, title, and comment
comes with it. Point a fresh PicOrg install at the folder and it
re-indexes in a couple of minutes.
