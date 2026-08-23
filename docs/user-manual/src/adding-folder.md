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

- **Remembers the folder in its own database.** Nothing is written
  inside your folder — the database lives in Magpie's own workshop
  area (`%APPDATA%\com.magpie.app\magpie.db`). See
  [Where your data lives](./data-storage.md).
- **Looks in the folder and every folder inside it.** So adding
  `Pictures` also picks up `Pictures\2020`, `Pictures\Kids`, and so
  on. Video and document files are picked up too — see
  [Supported file formats](./file-formats.md).
- **Reads the built-in info** each file exposes: dimensions, camera,
  duration, page count, and so on.
- **Reads any tags or titles** already saved with the file (for
  example, tags you added earlier in Windows Explorer or Adobe
  Bridge) and imports them into Magpie's database. This is a
  one-time import.
- **Makes small thumbnail previews** for images so the grid loads
  instantly, even for very large libraries.

Your original files are **not moved, renamed, or changed** in any
way during this. Magpie only reads them.

### If the folder is on OneDrive, Dropbox, or a network share

Any drive works, including OneDrive, Dropbox, iCloud Drive, Google
Drive, and network shares. Magpie's database is stored on your local
PC in the Windows AppData folder, so cloud sync clients don't touch
it. If you use Magpie on more than one PC, each PC has its own
database — tag edits made on one don't automatically appear on the
other.

## Adding more folders

You can add as many folders as you like — one for your holidays, one
for family, one for work documents, whatever suits you.

Every folder you add appears under **Library › Folders** in the
left-hand sidebar. Click a folder name there to see only the files
inside that folder.

## Removing a folder

Right-click the folder name in the sidebar and choose **Remove from
library**. Magpie forgets about it — but **your files stay on your
computer, untouched**. If you later add the folder to Magpie again,
its files are picked up fresh, and any tags that were already saved
inside the files (from Windows Explorer, Lightroom, or a previous
Magpie session) are imported one more time. Tags that only lived in
Magpie's database for that folder are gone with the removal.

Removing a folder from Magpie is not the same as deleting it from
your PC.

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
