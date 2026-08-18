# Tags: adding, renaming, cleaning up

Tags are the primary organisational tool in PicOrg. They are simple,
flat strings — no hierarchy, no groups. Tag a photo `family`, `2024`,
and `beach`, and later ask for "everything tagged `beach` and `2024`"
in a couple of clicks.

## The tag sidebar

Every tag in your library is listed under **Tags** in the sidebar,
with a count of how many photos carry it. The list is sorted
alphabetically.

- **Click a tag** to filter the grid to only photos with that tag.
- **Right-click** for tag maintenance:
  - **Rename tag…** — updates every photo carrying that tag,
    including their XMP sidecars and embedded XMP.
  - **Delete tag** — removes the tag from every photo, but does not
    delete any files.

## Adding tags

The [Editing metadata](./editing-metadata.md) and
[Working on many photos at once](./multi-select.md) chapters cover
the two main entry points. In both cases:

- **Space** or **Enter** commits the current typed word as a tag.
- **Comma** commits it too (useful if you're pasting a comma-separated
  list).
- **Backspace** on an empty input deletes the last committed tag.
- Tags are case-insensitive. `Beach` and `beach` are the same tag.

## Autocompletion

While typing a tag, PicOrg shows a dropdown with tags already in your
library that match your prefix. Use <kbd>↑</kbd>/<kbd>↓</kbd> to
select and <kbd>Enter</kbd> to accept — that keeps naming consistent
across your library.

## Tag persistence

When you save a tag, PicOrg writes it to:

1. The photo's `.xmp` sidecar (`Photo.jpg` → `Photo.xmp`).
2. The photo's embedded XMP inside the JPEG itself (for JPEG sources).

That means:

- Windows Explorer's *Details* → *Tags* column shows PicOrg's tags.
- Adobe Lightroom, Bridge, and digiKam read PicOrg's tags and
  vice-versa.
- Files remain valid JPEG/PNG/etc — no format-level surprises.

## Renaming tags safely

Right-click **Tags › `old-name`** → **Rename tag…**. PicOrg:

1. Updates every affected photo's XMP.
2. Rewrites the FTS search index so search finds the new spelling.
3. Refreshes the sidebar counts.

All in a single transaction — either the whole rename succeeds or none
of it does.

## Deleting a tag globally

**Right-click a tag → Delete tag** removes the tag from every photo in
the library. This is not the same as deleting photos — the files stay
put, they just lose the tag.

## A note on Windows Explorer tags

Windows Explorer stores tags in a Windows-specific `LastKeywordXMP`
field alongside the standard `dc:subject`. PicOrg reads **both**, so
tags added in Explorer show up in PicOrg. Tags added in PicOrg go into
both fields, so they show up in Explorer too.
