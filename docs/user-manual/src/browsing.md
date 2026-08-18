# Browsing your library

The centre of the PicOrg window is a **virtualised image grid**. It can
scroll through hundreds of thousands of photos without slowing down —
only the tiles visible in the viewport are actually drawn.

## The grid

Each tile shows a thumbnail plus the filename and pixel dimensions.
Hover a tile to see the full path in a tooltip; click to open the
photo in the details panel on the right.

- **Click** — select a photo (replaces any previous selection).
- **Ctrl+click** — add/remove a photo from the current selection.
- **Shift+click** — select a range from the last-clicked photo to the
  clicked one.
- **Double-click** — (reserved for future full-screen preview.)
- **Delete key** — move the current selection to the Recycle Bin,
  after a confirmation prompt.

The count in the top-left of the grid header (`152 images`) is the
number of photos that match the current filter, not necessarily the
whole library.

## Sorting

The **Sort** control at the top-right of the grid controls the order:

- **Taken** — by the date the photo was captured (from EXIF
  `DateTimeOriginal`). Missing dates fall to the end.
- **Modified** — by file system modification time.
- **Filename** — alphabetical.
- **Rating** — 5 stars first, unrated last.
- **Random** — a shuffled order that stays stable while the filter
  doesn't change.

Toggle the arrow next to the sort control to flip between ascending
and descending.

## The details panel

The right-hand pane shows details for the currently selected photo (or
photos, if more than one is selected). When nothing is selected, it
prompts you to pick something.

You can hide the details panel via the icon in the top-right corner to
give the grid more room. Toggle it back on from the same spot.

## Sidebar filters

The sidebar on the left has three sections that all act as filters:

1. **Library › Folders** — click a folder to filter to just its
   photos.
2. **Rating** — click a rating threshold to show photos with **that
   many stars or more**. So clicking "3 stars and up" shows 3-, 4-,
   and 5-star photos.
3. **Tags** — click a tag to filter to only photos with that tag.
   Numbers on the right show how many photos are in each tag.

Filters compose: click a folder **and** a rating **and** a tag and
you'll see only photos matching all three. Combine with the search box
for arbitrarily narrow queries.

## Clearing filters

Click **All photos** at the top of the sidebar to reset every filter
in one shot.
