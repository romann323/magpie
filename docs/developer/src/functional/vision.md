# Product vision and scope

## Vision

Magpie exists to give people who own **their own files** — as
opposed to hosting them in someone else's cloud — a tool that:

- Reads their files from disk, unchanged, wherever they are, and
  **never modifies the bytes.**
- Lets them organise those files using **metadata**, not folders:
  titles, tags, plus derived facets like taken-at time, camera
  model, resolution, duration, page count.
- Persists user tags in a small database **inside each library
  folder** (`.magpie/library.db`) that travels with the folder if
  the user copies or moves it.
- Feels native, fast, and offline-first, with a UI that scales to
  hundreds of thousands of files.

## Scope for v1

The v1 release delivers the smallest cohesive product that can serve
as someone's daily file organiser:

| Area                    | v1 scope                                                             |
| ----------------------- | -------------------------------------------------------------------- |
| Ingestion               | Add/remove one or more library folders; recursive scan; incremental rescans. |
| Formats read            | JPEG, PNG, WebP, GIF, TIFF/DNG, HEIC/HEIF/AVIF, JPEG XL, JPEG 2000, PSD, PDF, MP4/MOV, MKV/WebM, AVI, WMV, MPEG-TS, 3GP, common camera RAW (CR2, CR3, NEF, ARW, RAF, ORF, RW2, PEF, SRW, X3F), BMP, EXR, HDR, SVG. Legacy XMP sidecars are also read on first scan for backward compatibility. |
| Tag storage             | Per-folder SQLite (`.magpie/library.db`). Every recognised file type is fully taggable. |
| Metadata edited         | Title + tags.                                                        |
| Browsing                | Virtualised grid, sort by taken/added/filename/size.                 |
| Filtering               | Folder, tag, and full-text search (FTS5, cross-folder).              |
| Batch operations        | Bulk add/remove tags, bulk delete-to-recycle-bin.                    |
| Interoperability        | One-shot read of existing XMP / Windows Shell keywords into the folder DB on first scan; no write-back to source files. |
| Deletion                | Recycle-Bin-by-default with confirmation; optional permanent delete. |
| Storage                 | Two-tier SQLite (central `registry.db` in `%APPDATA%`, per-folder `library.db` inside each folder) + WebP thumbnail cache under `%APPDATA%`. |

## Product principles

1. **Never touch source files.** Not pixels, not XMP, not the
   Windows Shell property store. The bytes on disk are exactly what
   the camera / editor produced.
2. **Never move or rename files.** The user is in charge of their
   folder hierarchy.
3. **Tags travel with the folder.** Copying the folder to another
   disk carries its `.magpie/library.db` — and therefore its tags —
   along.
4. **No cloud, no telemetry.** Everything runs locally. There are
   no analytics, crash reports, or "phone home" mechanisms.
5. **Fast enough to be daily-drivable.** All hot paths are on
   native Rust; the UI is virtualised; per-folder DBs stay small
   even for very large libraries.
6. **Predictable failure modes.** A partially-broken run is better
   than a silently-lost edit. Every DB write is transactional,
   every batch op is per-item retryable, and the log records what
   happened.
