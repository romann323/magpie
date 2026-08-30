# The Magnifier

The Magnifier is a **separate pop-up window** that shows the current
picture at full size. It runs alongside the main Magpie window, so
you can keep browsing the grid while the Magnifier stays open on a
second monitor (or dragged out of the way on a single monitor).

## Opening it

- **Double-click** any tile in the grid — the fastest path.
- Select a tile and choose **View → Magnifier** (or press F11).
- If the details panel is showing a preview, **double-click that
  preview**.

If you trigger the Magnifier again while the pop-up is already open,
it jumps to the newly-chosen picture in the existing window instead
of opening a second one.

**View → Magnifier is greyed out until you select a file** — there
has to be something to magnify.

## Using it

| Do this           | And…                                          |
| ----------------- | --------------------------------------------- |
| **← / ↑**         | Show the previous file in the current filter. |
| **→ / ↓**         | Show the next file in the current filter.     |
| **Esc**           | Close the Magnifier window.                   |
| **Click "Close"** | Same as Esc.                                  |

Along the bottom of the pop-up you'll see the file's title (or its
filename if there's no title), your position in the list (for
example, "34 / 128"), and prev / next / close buttons. The window
title updates to the current filename as you navigate.

## Which files can I flip through?

Whatever was visible in the grid **at the moment you opened the
Magnifier** — Magpie walks through the same list, in the same order,
using your current sort. If you have tag chips or free-text search
active, the arrows stay within that filtered subset. Changing the
filter in the main window afterwards doesn't reshuffle the Magnifier;
close and re-open it to pick up the new list.

## Non-image files

The Magnifier only knows how to display picture formats your PC can
draw (JPEG, PNG, WEBP, GIF, BMP, TIFF). For a PDF or video the
Magnifier will show a "no image to display" message — open those
files in their own app instead.
