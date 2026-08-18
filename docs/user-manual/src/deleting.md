# Deleting photos

PicOrg has one deletion path with two safety layers: an explicit
confirmation prompt, and Recycle-Bin-by-default (so mistakes are
recoverable).

## Deleting a single photo

Select the photo, then either:

- Click the red **Delete image** button at the bottom of the details
  panel, or
- Press <kbd>Delete</kbd> on the keyboard.

A confirmation dialog appears:

> Move "IMG_2043.jpg" to the Recycle Bin?
> You can restore it from the Windows Recycle Bin afterwards.

Click **Move to Recycle Bin** to proceed, or **Cancel** to back out.

## Deleting many photos

Multi-select as usual (Ctrl+click, Shift+click) and click **Delete N
images** at the bottom of the details panel. The confirmation dialog
reads *"Move N images to the Recycle Bin?"*.

## What PicOrg actually does

On confirmation, for every selected photo, PicOrg:

1. Moves the source file to the **Windows Recycle Bin** using the
   OS API (the same code path Explorer uses).
2. Moves the sidecar `.xmp` file (if any) to the Recycle Bin too.
3. Removes the photo from the library database.
4. Deletes any cached thumbnails.

If a step fails (e.g. Windows refuses to move a locked file), PicOrg
does *not* remove the photo from the index — better to have a dead
row you can retry than lose track of a real file.

## Recovering a deleted photo

Open the Windows Recycle Bin, right-click the file, choose **Restore**.
It goes back where it was. On the next PicOrg **Rescan** the photo
reappears in the library with the same tags, rating, and title it had
before (they were preserved in the sidecar).

## Permanently deleting

If you're absolutely sure, hold <kbd>Shift</kbd> when clicking Delete
(or Shift+Delete on the keyboard). PicOrg then bypasses the Recycle
Bin and deletes the file directly. The confirmation prompt is worded
differently in that mode so you know you're skipping the safety net.

> **Warning.** Shift-delete cannot be undone. If in doubt, use the
> normal delete and empty the Recycle Bin later.

## What is NOT deleted

- Photos in **library folders you removed** are not touched — removing
  a library folder only removes the index entries.
- Tags "belong to the library"; even if you delete every photo that
  had a tag, the tag itself lingers in the sidebar with a count of 0
  until you right-click it → Delete tag.
