# Working on many files at once

Editing files one at a time is fine when there are only a few. But
when you have hundreds — say, an entire holiday — you'll want to
tag them all in one go. Magpie is built for exactly that.

## Picking several files

- **Click** the first file.
- **Ctrl + click** each of the other files you want to include.
- Or **Shift + click** at the far end to grab everything in between.
- **Ctrl + A** grabs every file in the current view.

The details panel on the right immediately switches to a batch view
that reads *"N files selected"*. The number ticks up as you add more.

## Adding tags to all of them

Batch mode shows two tag boxes:

- **Add tags** — anything you put here gets added to every selected
  file. Files that already had the tag are simply left alone.
- **Remove tags** — anything you put here gets removed from every
  selected file.

You can queue up multiple tags before hitting the button. When you're
ready, click **Apply tag changes to N files**. A confirmation appears
when it's done: *"Updated N files."*

> **Tip.** Add tags in bulk first, then use the sidebar to check
> your work. Click the tag on the left — the grid narrows to just
> the files that have it, and you can see whether everything landed
> correctly.

## Deleting them all at once

At the bottom of the details panel there's a red **Delete N files**
button. Click it, confirm the prompt, and the whole selection heads
to the Recycle Bin. See [Deleting files](./deleting.md) for the full
story.

## What if one file won't accept a change?

Two cases where a file gets skipped:

1. **The folder is read-only** (an old backup drive, or something you
   pulled off a DVD). Fix the permissions on that folder (right-click →
   *Properties* in File Explorer) and try again.
2. **The file's format doesn't support embedded tags yet** (RAW,
   HEIC, PDF, video…). Magpie still remembers the tag in its own
   library, but it can't write it back into the file. The rest of the
   batch gets updated normally.

Either way, Magpie shows a message at the end telling you which files
were skipped and why.
