# Working on many photos at once

Selecting more than one photo switches the details panel into
**batch mode**. The title bar shows `N images selected` and the
editable controls change to reflect what makes sense to apply in bulk.

## Selecting multiple photos

- **Ctrl+click** — toggle a photo in the current selection.
- **Shift+click** — extend the selection from the last-clicked photo
  to the new one.
- **Ctrl+A** — select everything currently visible in the grid.
- Click a photo without a modifier to reset the selection back to a
  single photo.

The selection persists as you scroll, so you can Ctrl+click a photo
near the top, scroll down thousands of pictures, and Shift+click one
at the bottom to grab a huge range.

## Bulk rating

Click a star in the **Set rating** control. That rating is applied to
every selected photo immediately — no confirmation.

Use rating `0` (the leftmost, empty-star click) to un-rate everything
in the selection.

## Bulk tag add / remove

The batch view has two tag inputs instead of one:

- **Add tags** — every tag you commit here is added to every
  selected photo, without touching any tags they already have.
- **Remove tags** — every tag you commit here is removed from every
  selected photo (photos that didn't have the tag are unaffected).

Both fields let you queue up multiple tags before hitting the button.
When you're ready, click **Apply tag changes to N images**. The button
label counts down as it saves and shows a green *"Updated N images"*
confirmation when done.

> **Tip.** In multi-select mode PicOrg doesn't show you the union of
> tags already on the selection — it would be misleading (some photos
> would have that tag, some wouldn't). Use single-select for that.

## Bulk delete

Click **Delete N images** at the bottom of the details panel to move
the whole selection to the Recycle Bin. A confirmation dialog appears
first — see the [Deleting photos](./deleting.md) chapter for details.

## What actually happens in bulk

Every bulk operation is applied **photo by photo** on the backend. If
one photo fails (say, its source folder is read-only) the others still
succeed. When there are failures, PicOrg summarises them in a dialog
after the run.
