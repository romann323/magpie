# Welcome to PicOrg

PicOrg is a fast, local, native photo-organiser for Windows (with future
macOS support). It lets you pull one or more folders of images into a
searchable library, tag and rate them, and organise them by metadata —
all without uploading a single byte to the cloud.

This manual walks you through everything you need to get productive with
PicOrg in an afternoon. If you're a developer looking for how the pieces
fit together, see the companion **PicOrg Developer Guide**.

## What PicOrg is (and isn't)

| PicOrg **is**                                                        | PicOrg **is not**                                            |
| -------------------------------------------------------------------- | ------------------------------------------------------------ |
| A metadata-first organiser: tags, ratings, titles, comments          | A photo editor (no crop, no retouch, no filters)             |
| A fast local viewer for large libraries (hundreds of thousands)      | A cloud service — no account, no upload, no telemetry        |
| Interoperable: writes standard XMP that Lightroom & Explorer read    | A DAM with keyword hierarchies or face recognition (yet)     |
| Free and open-source                                                 | A RAW developer                                              |

## Two-minute tour

1. **Add a folder** — Click **Add folder** in the top bar and pick a
   directory of images. PicOrg indexes it (scanning happens in parallel
   and updates the grid live).
2. **Browse** — Scroll through the virtualised grid on the left. Use
   the sidebar to filter by rating, tag, or folder.
3. **Edit** — Click any photo to open the details panel on the right.
   Type a title, click a star for a rating, add tags. Everything saves
   automatically.
4. **Multi-select** — Ctrl-click (Windows) to select multiple photos.
   The details panel switches to a batch view; add or remove tags,
   set a rating, or delete in bulk.
5. **Find** — Type in the search bar to filter across titles, comments,
   filenames, and tags.

That's it. The rest of this manual is optional deep-diving.

## Where things live at a glance

- **Your photos**: exactly where you put them. PicOrg never moves them.
- **Your metadata**:
    - Saved into an XMP **sidecar** file next to each photo
      (`Photo.jpg` → `Photo.xmp`), and
    - **Embedded inside the source JPEG** so tools like Windows
      Explorer, the Photos app, Adobe Lightroom, and digiKam see the
      same tags.
- **The PicOrg index**: a single SQLite database plus a thumbnail cache
  under `%APPDATA%\com.picorg.picorg\` — deleting that folder resets the
  app but never touches your photos.

The [Where your data lives](./data-storage.md) chapter has the full
story if you want to know exactly what changes.
