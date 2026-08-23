# Troubleshooting

Most bumps you'll hit fall into one of the buckets below. If none of
these matches your problem, Magpie keeps a diary in a log file that
can help — the last section explains where to find it.

## The app won't open

**What happens:** You double-click Magpie and either nothing happens
or you get a "WebView2" error message.

**Fix:** Install the free **Microsoft Edge WebView2 Runtime** from
Microsoft's website. It's a small download. Windows 11 already has
it; Windows 10 usually gets it automatically through Windows Update.

---

## Tags I added in File Explorer don't show up in Magpie

**What happens:** You right-click a file in File Explorer, add a
tag under **Properties › Details**, but Magpie doesn't list it.

**Why:** Magpie's tag data lives in a database inside the folder,
not inside the file. Explorer edits go into the file. Magpie reads
those file tags **once**, on first scan, and then treats its
database as the source of truth.

**Fix:** Delete the folder's `.magpie\library.db` file (or rename
it) and add the folder to Magpie again. The first scan re-imports
whatever tags Explorer, Bridge, or Lightroom has put in the files.
Any tags you'd already added in Magpie for that folder are lost by
this step, so use it only when the other tool is the authoritative
tagger.

---

## Tags I added in Magpie don't show up in File Explorer

**What happens:** You add a tag in Magpie, open the file's
properties in File Explorer, and the Tags row is empty.

**Why:** Magpie doesn't write into your files anymore. Tags live in
the folder's `.magpie\library.db` database.

**If you need the tag inside the file** (for example so it survives
being sent to a friend by email), tag the file with Explorer's
*Properties → Details* dialog directly, or with a tool like Adobe
Bridge / digiKam that writes XMP. Magpie will pick that tag up the
next time you delete `.magpie\library.db` and rescan the folder.

---

## Files are missing from the grid

**What happens:** A folder has 500 files in File Explorer but only
480 appear in Magpie.

**Common reasons:**

- **Unusual file formats.** Magpie handles the ones listed on
  [Supported file formats](./file-formats.md). Others are skipped.
- **Hidden files.** Files marked as Hidden or System in Windows are
  skipped on purpose.
- **A previous scan hit an error.** Right-click the folder →
  **Rescan folder** to try again.

---

## The folder shows "(offline)" in the sidebar

**What happens:** A folder that used to work now appears greyed-out
with an *(offline)* label.

**Why:** Magpie couldn't find the folder's `.magpie\library.db`
file. Usually this is because the drive is unplugged (external
disk) or a network share is not reachable right now.

**Fix:** Plug the drive back in / reconnect to the network share.
Restart Magpie (or use **Rescan** at the top). The folder should
come back online.

---

## The app is slow / the grid stutters

**What to try:**

- **Wait for the first scan.** The very first time Magpie reads a
  folder it works your CPU hard making thumbnails. Give it a few
  minutes. Next time you open the app, it'll be instant.
- **Close other heavy apps.** Magpie uses your graphics card to draw
  the grid smoothly. If another app is eating that up, both apps
  suffer.
- **Restart Magpie.** Sometimes the simplest fix.

---

## "Save failed" message

**What happens:** After adding a tag, a red **Save failed** message
appears in the details panel.

**Fix:** Most common causes:

1. **The folder is read-only** — Magpie can't write into the
   `.magpie\library.db` file. Check the folder's *Properties → General*
   and turn off *Read-only*.
2. **Two Magpie windows fighting over the same folder.** Close the
   other instance and try again.
3. **Disk full.** The database is tiny but SQLite needs a few
   kilobytes free to write.

The log file (see below) will tell you exactly which folder failed.

---

## "This folder is on OneDrive / Dropbox / a network share"

**What happens:** When you add a folder, Magpie shows a warning
naming the sync provider.

**Why:** The folder lives on a cloud-synced disk or a network share,
so two PCs could theoretically open the same folder in Magpie at
the same time and both write into `.magpie\library.db` — the sync
client can't merge those writes.

**Answer:** Add the folder anyway if only **one PC at a time** uses
Magpie on it. That's the common case (laptop away from the desktop,
etc.) and it's safe.

---

## Resetting Magpie

If Magpie gets into a weird state and you want to start fresh:

1. Close Magpie.
2. Open File Explorer, paste `%APPDATA%\com.magpie.app` into the
   address bar, and press Enter.
3. Delete that folder.
4. Reopen Magpie — the folder list is empty, so re-add your
   folders. **Your tags survive** — they live inside each folder's
   `.magpie` subfolder.

---

## Finding the log file

Magpie keeps a diary of everything it does at:

```
%APPDATA%\com.magpie.app\logs\app.log
```

Open it in Notepad. The bottom of the file is the most recent
activity. Lines beginning with `WARN` or `ERROR` are the interesting
ones if something went wrong. This is the file to share (or paste
into) if you ever report a bug.
