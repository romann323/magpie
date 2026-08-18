# Editing metadata

Select a single photo in the grid to open the **Details** panel on
the right. It shows every editable field along with read-only
information about the file itself.

## Editable fields

| Field    | Standard XMP mapping                 | Notes                                        |
| -------- | ------------------------------------ | -------------------------------------------- |
| Title    | `dc:title` / `xmp:Title`             | Free-form text, single line.                 |
| Rating   | `xmp:Rating` (0–5)                   | 0 = "not rated". Click a star to set.        |
| Tags     | `dc:subject` + `MicrosoftPhoto:LastKeywordXMP` | Space, comma, or Enter commits a new tag. |
| Comment  | `dc:description` / `xmp:Description` | Free-form text, multi-line.                  |

## Auto-save

**PicOrg saves automatically as you type.** There is no explicit
"Save" button:

- **Title and comment** save 600 ms after your last keystroke (or
  immediately if you Tab away from the field). The details panel title
  briefly reads `Saving title…` while it's happening.
- **Rating and tags** save the moment you commit the change (click a
  star, or press Space/Enter after a tag).

Every save writes to **two** places:

1. The `.xmp` sidecar next to the photo.
2. The **embedded XMP** inside the source JPEG itself.

So a title you type in PicOrg shows up in Windows Explorer's Details
tab and in Lightroom without any further action.

## Clearing a field

- **Title / Comment:** delete the text and Tab out (or wait for the
  auto-save).
- **Rating:** click the current star again to reset back to
  "not rated".
- **Tags:** hover a tag pill and click the `×` that appears; that tag
  is removed immediately.

## Tag input specifics

The tag input has one job — accepting strings — but the small details
matter:

- Type a tag, then press **Space**, **Comma**, or **Enter** to commit
  it into a chip.
- Clicking elsewhere (blurring the input) also commits any pending
  draft; you won't lose a typed tag by clicking Save-adjacent
  buttons.
- Autocompletion suggests existing tags in your library while you
  type. Use the arrow keys and Enter to pick one.
- Backspace on an empty input deletes the last committed tag.
- Tags are case-insensitive: adding `Beach` to a photo that already
  has `beach` is a no-op.

## Read-only info

The **File info** block underneath the editable fields shows:

- Pixel dimensions.
- File size in a human-readable unit.
- Taken time (from EXIF `DateTimeOriginal`).
- Modified time (from the file system).
- Camera make and model (from EXIF).
- Format (JPEG, PNG, HEIC, RAW, …).
- **Metadata saved** — the last time PicOrg wrote XMP for this photo.

The path is displayed above the file info; click it to select and copy
the full string to your clipboard.
