# Editing metadata

**Metadata** is a fancy word for "information about a file" — its
title and tags. It's what makes a folder of `IMG_1234.jpg` and
`document_final_v3.pdf` files feel like a real library.

To edit any of this, click one file in the grid. The **details
panel** on the right lights up with everything you can change.

## What the panel shows

The details panel is split into four sections, top to bottom:

1. **Title** — a short, editable name (e.g. "Anna's birthday cake").
2. **Tags** — little labels like `beach`, `family`, `2024`,
   `contract`.
3. **Format metadata** — which handler Magpie is using for this file
   (JPEG, PNG, PDF, MP4, …) and whether Magpie can save your edits
   directly into the file.
4. **File info** — everything Magpie learned from the file itself:
   size, resolution, when it was taken, camera used, page count, video
   duration, and so on. This section is read-only — those numbers
   describe the file, not your notes about it.

## Everything saves as you type

There is **no Save button**. Magpie saves your changes automatically:

- **Title**: saved a moment after you stop typing (or as soon as you
  click somewhere else).
- **Tags**: saved immediately once you press space, Enter or comma.

You'll see a tiny "Saving…" note when it's happening. It usually
takes less than half a second.

## Setting a title

Click the **Title** box, type a name, click somewhere else (or wait a
moment). Done.

To clear a title, just erase what's there.

## Adding tags

Click the **Tags** box. Type a word — for example `family` — and then
press one of:

- **Space**
- **Enter**
- **Comma**

The tag becomes a little pill you can see. Type another word, press
space, and there's another. Keep going for as many tags as you want.

While you type, Magpie suggests tags you've already used elsewhere,
so you don't end up with `Beach`, `beach`, and `beaches` all meaning
the same thing. Use the arrow keys to pick a suggestion and press
Enter.

To **remove a tag**, hover over its pill and click the little **×**.

## Where the tags actually live

Magpie always saves your tags into its **library index** so search
and filtering stay fast — even for files on drives that aren't
plugged in right now.

On top of that, Magpie also tries to save your tags **inside the
file itself**. There are three possible outcomes, and the tag editor
tells you which one is happening for the file you're looking at:

1. **Embedded directly by Magpie (XMP).**
   For JPEG, PNG, WebP and GIF89a, Magpie writes the tags right into
   the image file using the industry-standard *XMP* slot. Adobe
   Bridge, Lightroom, Photos, and File Explorer all see them.
   > *Title and tags are embedded directly into the JPG file (XMP).*
2. **Saved by Windows on Magpie's behalf.**
   For RAW photos (CR2/CR3/NEF/ARW/DNG/RAF/RW2/…), HEIC/HEIF/AVIF,
   TIFF, MP4/MOV/M4V/3GP, WMV/ASF, and JPEG XR/JXR, Magpie hands the
   tags to the exact same Windows property system that Explorer's
   *Properties → Details* tab uses. Anything you tag in Magpie shows
   up in Explorer immediately, and vice-versa.
   > *Title and tags are saved on the source file via the same
   > Windows property system that Explorer's Properties → Details
   > tab uses.*
3. **Library-only (fallback).**
   A handful of formats have no writable property handler on Windows
   — BMP, DIB, SVG, EXR, HDR, JPEG 2000, PSD, and older MPEG-TS
   variants are the main ones. Your tags are still saved in
   Magpie's library, but they won't travel with the file if you
   copy it elsewhere.
   > *Windows has no property handler that can embed tags in BMP
   > files on this system, so tags are stored in Magpie's library
   > only.*

See [Supported file formats](./file-formats.md) for the full
per-format table.

## If a save fails

If Windows or the file itself rejects the write (usually because the
file is read-only, on a locked drive, or open in another program),
the panel shows a red note under the Tags field:

> Save failed: *(the exact error from Windows)*

Fix the underlying problem — close the other program, take the file
off read-only, or move it somewhere writable — and re-edit the tag.
Magpie's library is *not* updated for a failed save, so no state is
ever silently out of sync.
