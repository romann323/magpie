# Adding your first folder

PicOrg organises photos around **library folders**. A library folder is
just any directory on your disk that you'd like PicOrg to keep an eye
on — the app scans it recursively and shows every image it finds in the
grid.

## Adding a folder

1. Click **+ Add folder** in the top bar.
2. Pick a folder in the system dialog. Windows OneDrive folders,
   network shares, and long-path (`\\?\`) paths are all supported.
3. Scanning starts immediately. The status bar at the bottom shows
   `Scanning N photos…` and updates in real time.

The grid populates as PicOrg discovers files: you can start browsing
and editing before the scan is finished.

## What "scanning" actually does

For every image file, PicOrg:

1. Records its full path, size, modification time, and format.
2. Reads embedded EXIF for taken-at time, camera make/model, and image
   dimensions.
3. Reads XMP (from the sidecar `.xmp` **or** from an APP1 segment inside
   the JPEG itself) for title, rating, tags, and comment.
4. Generates two thumbnail sizes (small and medium) into the cache.
5. Computes a content hash so PicOrg can detect duplicates and
   renames later.

Scanning uses all your CPU cores and is bounded by disk read speed. A
first-time scan of 10 000 photos on an SSD takes a couple of minutes;
subsequent rescans are near-instant because PicOrg only re-processes
files whose modification time has changed.

## Adding more folders

You can add as many library folders as you like. They all show up
under **Library › Folders** in the sidebar. Click a folder name to
filter the grid to just its photos.

## Removing a folder

Right-click a folder in the sidebar and choose **Remove from library**.
This removes it from the PicOrg index and deletes the corresponding
thumbnails. **Your photos and sidecar `.xmp` files are untouched.**

## Rescanning

Photos added to a library folder outside of PicOrg (say, you dropped
new files in with File Explorer) aren't discovered automatically. Two
ways to fix that:

- **Right-click a folder → Rescan** to rescan just that folder.
- **Click Rescan in the top bar** to rescan every folder in the
  library.

Rescans are incremental and safe: PicOrg only touches files that have
changed, are new, or have disappeared.
