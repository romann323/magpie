# Adding your first folder

Magpie needs to know which folders on your computer contain the files
you want to organise. Once you tell it, it does the rest.

## Add a folder in three clicks

1. Click **+ Add folder** at the top of the window.
2. In the dialog that opens, browse to the folder that has your
   files — for example `C:\Users\me\Pictures\Vacation 2024`.
3. Click **Select folder**.

That's it. Magpie starts looking through the folder right away. The
bar at the bottom of the window shows what's happening
("Reading 234 of 1,205 files…") and updates as it goes.

You don't have to wait for it to finish — files appear in the
window as they're found, so you can start browsing (and even tagging)
straight away.

## What Magpie does when you add a folder

Behind the scenes, Magpie:

- **Looks in the folder and every folder inside it.** So adding
  `Pictures` also picks up `Pictures\2020`, `Pictures\Kids`, and so
  on. Video and document files are picked up too — see
  [Supported file formats](./file-formats.md).
- **Reads the built-in info** each file exposes: dimensions, camera,
  duration, page count, and so on.
- **Reads any tags or titles** already saved with the file (for
  example, tags you added earlier in Windows Explorer).
- **Makes small thumbnail previews** for images so the grid loads
  instantly, even for very large libraries.

Your original files are **not moved, renamed, or changed** in any
way during this. Magpie only reads them.

**If you turned on [Auto-tag photos](./settings.md#auto-tag-photos)**
in Settings, Magpie also runs its built-in tagger on each photo right
after the scan finishes. You'll see a second green
**Auto-tagging** progress bar in the status bar at the bottom of
the window. The suggested tags land in each photo's **Automatic
tags** section (read-only) so it's obvious a machine picked them.
This step is off by default.

## Adding more folders

You can add as many folders as you like — one for your holidays, one
for family, one for work documents, whatever suits you.

Every folder you add appears under **Library › Folders** in the
left-hand sidebar. Click a folder name there to see only the files
inside that folder.

## Removing a folder

Right-click the folder name in the sidebar and choose **Remove from
library**. Magpie forgets about it — but **your files stay on your
computer**. Removing a folder from Magpie is not the same as deleting
it from your PC.

## Refreshing when files change

If you drop new files into a folder using File Explorer, or edit
some in another app, Magpie doesn't know about it right away. To
catch up:

- **Right-click a folder name** in the sidebar and choose **Rescan
  folder**, or
- Click **Rescan** at the top of the window to check every folder
  at once.

Rescanning is quick — Magpie only looks at files that have changed
since the last time it checked.
