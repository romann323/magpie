# Troubleshooting

Most issues fall into one of a handful of buckets. The log at
`%APPDATA%\com.picorg.picorg\logs\picorg.log` records what PicOrg
did — open it in Notepad, scroll to the bottom, and look for
`WARN` or `ERROR` lines.

## The app won't start

**Symptom:** Double-click PicOrg, nothing happens, or a WebView2
error dialog appears.

**Fix:** Install the Microsoft Edge WebView2 Runtime. It's a free
Microsoft download. Windows 11 already has it; Windows 10 usually
gets it via Windows Update.

---

## Tags I set in Windows Explorer don't show up

**Symptom:** You added tags in File Explorer's Details panel, but
PicOrg's sidebar doesn't list them.

**Cause:** PicOrg reads metadata on the first scan and caches it.
Explorer wrote new tags into the JPEG *after* that.

**Fix:** Click **Rescan** in the top bar, or right-click the folder
containing the photos and choose **Rescan folder**. PicOrg detects
the modified files and re-reads their metadata.

---

## Tags I set in PicOrg don't show up in Windows Explorer

**Symptom:** You added a tag in PicOrg, but Explorer's *Tags* column
is blank.

**Diagnosis and fix — one of the following:**

1. **You need to refresh Explorer's index.** Explorer caches tag
   listings aggressively; F5 doesn't always rebuild them. Right-click
   the folder → **Refresh** or navigate away and back.
2. **The file is a non-JPEG format.** PicOrg embeds tags into JPEG
   only in v1; for PNG/HEIC/RAW it writes a sidecar (which Explorer
   doesn't read). Convert to JPEG or use the sidecar-aware tool of
   your choice.
3. **Read-only file or folder.** PicOrg logs a warning like
   `sidecar write failed`. Grant write permission to the folder and
   retry.

---

## Photos are missing from the grid

**Symptom:** A folder has photos on disk but PicOrg's grid shows
fewer.

**Possible causes:**

- **Unsupported format.** PicOrg indexes JPEG, PNG, GIF, BMP, TIFF,
  WebP, HEIC/HEIF, and common RAW extensions (CR2, CR3, NEF, ARW,
  DNG, RAF, ORF, PEF, X3F). Other files are ignored.
- **Files are hidden or system-flagged.** PicOrg skips OS-hidden
  files by default.
- **A prior scan errored.** Run **Rescan folder** to retry.

---

## The app is slow to start / laggy scrolling

**Symptom:** First launch takes noticeable time, or the grid stutters.

**Fix:**

- The first scan of a large folder generates thumbnails on all CPU
  cores. Wait for the status bar to say `Idle` — subsequent launches
  are near-instant.
- If scrolling is choppy, close other GPU-heavy apps; PicOrg uses
  WebView2 which shares the GPU.

---

## "Save failed" toast in multi-select

**Symptom:** After clicking *Apply tag changes to N images*, PicOrg
shows a red *Save failed: …* message.

**Fix:** Open `picorg.log` and look for the line right after
`applyTags dispatch:`. It'll say specifically which image failed
(usually a permission issue on the source folder). Fix the file
permissions and click Apply again.

---

## I want to reset PicOrg completely

Close the app. Delete `%APPDATA%\com.picorg.picorg\`. Relaunch. All
your photos and sidecars are still on disk — PicOrg re-scans on the
next launch.
