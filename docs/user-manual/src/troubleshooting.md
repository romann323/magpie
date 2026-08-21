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
tag under **Properties › Details**, but Magpie's sidebar doesn't
list it.

**Why:** Magpie only re-reads tags when you tell it to (or when you
click the file). It doesn't watch your folders in the background.

**Fix:** Click **Rescan** at the top of Magpie, or right-click the
folder in the sidebar and choose **Rescan folder**. The tag will
appear.

---

## Tags I added in Magpie don't show up in File Explorer

**What happens:** You add a tag in Magpie, open the file's
properties in File Explorer, and the Tags row is empty.

**Fixes to try, in order:**

1. **Refresh the Explorer window.** Press **F5** or navigate away and
   back. Explorer likes to hold onto old data.
2. **Check the file format.** Only formats that Magpie can write into
   propagate tags to Explorer — see
   [Supported file formats](./file-formats.md). Convert to JPEG or
   PNG if you need Explorer to see the tag.
3. **Check the folder isn't read-only.** If the folder is on a
   read-only drive (like a locked backup), Magpie can't save inside
   the file. Right-click the folder → **Properties** and make sure
   *Read-only* is off.

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

**What happens:** After clicking **Apply tag changes to N files**,
a red **Save failed** message pops up.

**Fix:** Two common causes:

1. **Folder permissions.** One of the files lives on a network
   drive or locked backup. Check that you can write to that folder
   in File Explorer, then try again.
2. **The file is in a format Magpie can't tag inside yet.** For
   those, the tag lives in Magpie's library only, but the batch
   won't show a red error — it just skips the file with a note.

The log file (see below) will tell you exactly which file failed.

---

## "This format can't store Magpie tags inside the file"

**What happens:** You try to edit a `.cr2` RAW file or a `.heic`
photo and get this message.

**Why:** Only a handful of image formats (JPEG, PNG, WebP, GIF89a)
have a safe, standard way to embed tags. Magpie won't create hidden
`.xmp` companion files, so if it can't put your edit *inside* the
file, it won't pretend to save it there.

**Workarounds:**

- Tag the file anyway — it works. The tag is remembered in Magpie's
  library. It just won't travel with the file to another computer.
- Or convert (or export a copy of) the file to JPEG/PNG/WebP/GIF and
  tag that copy.
- If you shoot RAW+JPEG, tag the JPEG version — Lightroom, Bridge,
  and other tools will read those tags too.

---

## Resetting Magpie

If Magpie gets into a weird state and you want to start fresh:

1. Close Magpie.
2. Open File Explorer, paste `%APPDATA%\com.magpie.app` into the
   address bar, and press Enter.
3. Delete that folder.
4. Reopen Magpie — it starts empty and you can add your folders
   again.

**Your files and any tags saved inside them are all safe** — those
live in your folders, not in the folder you just deleted. Tags that
Magpie only kept in its own library are lost, so this is a proper
reset.

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
