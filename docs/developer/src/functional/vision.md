# Product vision and scope

## Vision

PicOrg exists to give people who own **their own photos** — as
opposed to hosting them in someone else's cloud — a tool that:

- Reads their images from disk, unchanged, wherever they are.
- Lets them organise those images using **metadata**, not folders:
  ratings, tags, titles, comments, plus derived facets like taken-at
  time, camera model, and pixel dimensions.
- Persists that metadata in **standard, interoperable formats**
  (XMP inside the file, and sidecar XMP next to it) so nothing is
  locked into PicOrg.
- Feels native, fast, and offline-first, with a UI that scales to
  hundreds of thousands of photos.

## Scope for v1

The v1 release delivers the smallest cohesive product that can serve
as someone's daily photo organiser:

| Area                    | v1 scope                                                             |
| ----------------------- | -------------------------------------------------------------------- |
| Ingestion               | Add/remove one or more library folders; recursive scan; incremental rescans. |
| Formats read            | JPEG, PNG, GIF, BMP, TIFF, WebP, HEIC/HEIF, common RAW (CR2, CR3, NEF, ARW, DNG, RAF, ORF, PEF, X3F). |
| Formats written         | JPEG (embedded XMP + sidecar), everything else (sidecar only).       |
| Metadata edited         | Title, rating (0..5), tags, comment.                                 |
| Browsing                | Virtualised grid, sort by taken/modified/filename/rating/random.     |
| Filtering               | Folder, rating threshold, tag, and full-text search (FTS5).          |
| Batch operations        | Bulk rate, bulk add/remove tags, bulk delete-to-recycle-bin.         |
| Interoperability        | XMP round-trip with Lightroom, Bridge, digiKam, Windows Explorer.    |
| Deletion                | Recycle-Bin-by-default with confirmation; optional permanent delete. |
| Storage                 | SQLite index + WebP thumbnail cache under `%APPDATA%`.               |

## Product principles

1. **Never modify pixels.** Metadata edits go into the XMP segment
   of the file (or a sidecar), never into the image data itself.
2. **Never move or rename files.** The user is in charge of their
   folder hierarchy.
3. **No cloud, no telemetry.** Everything runs locally. There are no
   analytics, crash reports, or "phone home" mechanisms.
4. **Fast enough to be daily-drivable.** All hot paths are on native
   Rust; the UI is virtualised; the DB fits in RAM even for
   very large libraries.
5. **Interop over lock-in.** If a user decides to leave PicOrg, all
   their metadata is already in industry-standard files right next
   to their photos.
6. **Predictable failure modes.** A partially-broken run is better
   than a silently-lost edit. Every write is atomic, every batch
   op is per-item retryable, and the log records what happened.
