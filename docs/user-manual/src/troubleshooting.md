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

**Why:** Magpie's tag data lives in its own database, not inside
the file. Explorer edits go into the file. Magpie reads those file
tags **once**, on first scan of the folder, and then treats its
database as the source of truth.

**Fix:** Right-click the folder in the sidebar and choose **Remove
from library**, then add the folder again. The first scan re-imports
whatever tags Explorer, Bridge, or Lightroom have put in the files.
Any tags you'd already added in Magpie for that folder are lost by
this step, so use it only when the other tool is the authoritative
tagger.

---

## Tags I added in Magpie don't show up in File Explorer

**What happens:** You add a tag in Magpie, open the file's
properties in File Explorer, and the Tags row is empty.

**Why:** Magpie doesn't write into your files. Tags live only in
Magpie's database.

**If you need the tag inside the file** (for example so it survives
being sent to a friend by email), tag the file with Explorer's
*Properties → Details* dialog directly, or with a tool like Adobe
Bridge / digiKam that writes XMP. Magpie will pick that tag up the
next time you re-add the folder to import fresh.

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

**Why:** Magpie couldn't reach the folder on disk. Usually this is
because the drive is unplugged (external disk) or a network share
is not reachable right now.

**Fix:** Plug the drive back in / reconnect to the network share.
Click **Rescan** at the top of the window. The folder should come
back online.

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

1. **Disk full.** The database is tiny but SQLite needs a few
   kilobytes free to write.
2. **Another Magpie window is running.** Close the other instance
   and try again — only one Magpie can safely write to the database
   at a time on the same PC.
3. **File permissions on `%APPDATA%`.** Very unusual. If your
   Windows account can't write to its own `AppData\Roaming` folder,
   most apps won't work.

The log file (see below) will tell you exactly what went wrong.

---

## Resetting Magpie

If Magpie gets into a weird state and you want to start fresh:

1. Close Magpie.
2. Open File Explorer, paste `%APPDATA%\com.magpie.app` into the
   address bar, and press Enter.
3. Delete `magpie.db` (or the whole folder — everything else can be
   regenerated).
4. Reopen Magpie. You start with an empty library; re-add your
   folders. Any tags that were embedded in the files themselves (by
   Explorer, Bridge, or Lightroom) come back automatically on first
   scan. Tags you'd only added inside Magpie are gone.

Tip: before step 3, copy `magpie.db` somewhere safe. That single
file is your whole library backup.

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
